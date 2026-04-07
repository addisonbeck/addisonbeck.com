;;; org-roam-export.el --- Export org-roam DB to JSON  -*- lexical-binding: t -*-
;;; Requires Emacs 27.1+ for proper-list-p
;;; Commentary:
;;  Export entire org-roam database to sharded JSON files.
;;  One <id>.json file is written per node into subdirectories
;;  sharded by the first two characters of the node ID, plus a
;;  manifest.json at the output root containing DB-level fields
;;  for all nodes (no AST).
;;
;;  Invoke in batch mode via org-roam-export--batch-main, which reads
;;  configuration from environment variables. See that function's
;;  docstring for details.
;;; Code:

(require 'json)
(require 'cl-lib)

(defun org-roam-export--serialize-ast (root)
  "Serialize org-element ROOT to a JSON-safe Elisp structure.
Uses an explicit work stack to avoid specpdl depth exhaustion on
deeply-nested org-element ASTs.  :parent pointers are stripped to
prevent circular-structure traversal.  A seen-objects hash table
detects any remaining cycles (e.g. from org-element--deferred closures).
Requires Emacs 27.1+ for `proper-list-p'."
  (let ((tasks (list (cons :eval root)))
        (vals  nil)
        (seen  (make-hash-table :test 'eq)))
    (while tasks
      (let* ((task (pop tasks))
             (op   (car task))
             (data (cdr task)))
        (pcase op
          ;; Push a pre-computed literal value onto the value stack
          (:push (push data vals))
          ;; Evaluate a node and push its serialized form onto vals
          (:eval
           (let ((node data))
             (cond
              ((null node)    (push nil vals))
              ((eq node t)    (push t vals))
              ((stringp node) (push (substring-no-properties node) vals))
              ((numberp node) (push node vals))
              ((keywordp node)(push (substring (symbol-name node) 1) vals))
              ((symbolp node) (push (symbol-name node) vals))
              ((bufferp node)
               (push nil vals))
              ((markerp node)
               (push (if (marker-buffer node)
                         `(("marker" . ,(marker-position node))
                           ("buffer" . ,(buffer-name (marker-buffer node))))
                       `(("marker" . ,(marker-position node))
                         ("buffer" . nil)))
                     vals))
              ((overlayp node)
               (push (format "#<overlay %d-%d>"
                             (overlay-start node) (overlay-end node))
                     vals))
              ;; Records (org-element--deferred, compiled-function, cl-defstruct
              ;; objects in Emacs 29+).  Treat as opaque to prevent traversal
              ;; into deferred closures.
              ((and (fboundp 'recordp) (recordp node))
               (push nil vals))
              ;; Closures: (closure ENV ARGS BODY).  The ENV may reference parent
              ;; AST nodes, creating a cycle.  Treat as opaque.
              ((and (consp node) (eq (car node) 'closure))
               (push nil vals))
              ;; Cycle detection: if we have already started serializing this
              ;; exact object (same eq identity), we have a circular reference.
              ((gethash node seen)
               (push nil vals))
              ;; Plist: (:key val :key val ...) -> alist, skipping :parent.
              ;; :standard-properties is a vector of common org-element
              ;; properties (begin, end, post-blank, etc.) introduced in
              ;; Emacs 29.  Expand it into individual key-value pairs so
              ;; properties like post-blank are visible to the renderer.
              ((and (listp node) (keywordp (car node)))
               (puthash node t seen)
               (let ((pairs nil) (rem node) (std-props nil))
                 (while rem
                   (let ((k (car rem)) (v (cadr rem)))
                     (cond
                      ((eq k :parent) nil)              ; always skip
                      ((eq k :standard-properties)
                       (setq std-props v))              ; expand below
                      (t (push (cons (substring (symbol-name k) 1) v) pairs))))
                   (setq rem (cddr rem)))
                 ;; Expand :standard-properties vector into individual pairs.
                 (when (and std-props
                            (vectorp std-props)
                            (boundp 'org-element--standard-properties))
                   (let ((keys org-element--standard-properties)
                         (i    0))
                     (while (and keys (< i (length std-props)))
                       (push (cons (substring (symbol-name (car keys)) 1)
                                   (aref std-props i))
                             pairs)
                       (setq keys (cdr keys))
                       (cl-incf i))))
                 ;; pairs is in reverse plist order (last entry first).
                 ;; For each pair: push :eval val first (deeper in task stack,
                 ;; executes later -> val ends up on TOP of vals), then :push key
                 ;; (executes sooner -> key ends up BELOW val on vals).
                 ;; :alist-collect pops val first, then key -> (key . val).
                 (push (cons :alist-collect (length pairs)) tasks)
                 (dolist (pair pairs)
                   (push (cons :eval (cdr pair)) tasks)
                   (push (cons :push (car pair)) tasks))))
              ;; Hash table -> alist (same task pattern as plist)
              ((hash-table-p node)
               (puthash node t seen)
               (let ((pairs nil))
                 (maphash (lambda (k v)
                            (push (cons (format "%s" k) v) pairs))
                          node)
                 (push (cons :alist-collect (length pairs)) tasks)
                 (dolist (pair pairs)
                   (push (cons :eval (cdr pair)) tasks)
                   (push (cons :push (car pair)) tasks))))
              ;; Vector -> serialize each element, collect as vector
              ((vectorp node)
               (puthash node t seen)
               (let ((lst (append node nil)))
                 (push (cons :vector-collect (length lst)) tasks)
                 (dolist (elt lst)
                   (push (cons :eval elt) tasks))))
              ;; Proper list (non-plist) -> serialize each element, collect as vector
              ((proper-list-p node)
               (puthash node t seen)
               (push (cons :vector-collect (length node)) tasks)
               (dolist (elt node)
                 (push (cons :eval elt) tasks)))
              ;; Cons cell -> two-element vector [car cdr]
              ((consp node)
               (puthash node t seen)
               (push '(:cons-collect) tasks)
               (push (cons :eval (car node)) tasks)
               (push (cons :eval (cdr node)) tasks))
              ;; Fallback: print representation
              (t (push (format "%S" node) vals)))))
          ;; Collect N values from vals into a vector (preserving element order)
          (:vector-collect
           (let ((items nil))
             (dotimes (_ data)
               (push (pop vals) items))
             (push (apply #'vector (nreverse items)) vals)))
          ;; Collect N key-value pairs from vals into an alist.
          ;; Val-stack layout per pair (top first): val, key.
          ;; Pop val first, then key -> (key . val).
          (:alist-collect
           (let ((result nil))
             (dotimes (_ data)
               (let ((v (pop vals))
                     (k (pop vals)))
                 (push (cons k v) result)))
             (push result vals)))
          ;; Collect two vals into a two-element vector [car cdr].
          ;; Val-stack: car on top (index 0), cdr below (index 1).
          (:cons-collect
           (let ((a (pop vals))
                 (b (pop vals)))
             (push (vector a b) vals))))))
    (car vals)))


(defun org-roam-export--node-to-json (node output-dir)
  "Export NODE to a JSON file in OUTPUT-DIR, sharded by first two ID chars."
  (condition-case err
      (let* ((id        (org-roam-node-id node))
             (file      (org-roam-node-file node))
             (shard     (substring id 0 2))
             (shard-dir (expand-file-name shard output-dir))
             (out-path  (expand-file-name (concat id ".json") shard-dir)))
        (message "  [diag] step 1: reading %s" file)
        (let ((serialized
               (with-temp-buffer
                 (insert-file-contents file)
                 (let* ((ast (org-element-parse-buffer))
                        (result (progn
                                  (message "  [diag] step 2: ast type=%S car-type=%S"
                                           (type-of ast) (type-of (car-safe ast)))
                                  (message "  [diag] step 2b: serializing")
                                  (org-roam-export--serialize-ast ast))))
                   (message "  [diag] step 2c: copying media")
                   (let ((image-exts '(".png" ".jpg" ".jpeg" ".gif" ".svg" ".webp" ".avif" ".pdf"))
                         (media-dir (expand-file-name (concat "media/" id) output-dir))
                         (org-dir (file-name-directory file)))
                     (org-element-map ast 'link
                       (lambda (link)
                         (when (string= (org-element-property :type link) "file")
                           (let* ((path (org-element-property :path link))
                                  (ext (downcase (or (file-name-extension path t) ""))))
                             (when (member ext image-exts)
                               (let* ((src (expand-file-name path org-dir))
                                      (dest (expand-file-name (file-name-nondirectory path) media-dir)))
                                 (if (file-exists-p src)
                                     (progn (make-directory media-dir t)
                                            (copy-file src dest t))
                                   (message "org-roam-export: WARNING missing media %s" src)))))))))
                   result))))
          (message "  [diag] step 3: querying links")
          (let* ((links-to
                  (mapcar #'car
                          (org-roam-db-query
                           [:select [dest] :from links
                            :where (= source $s1)
                            :and   (= type "id")]
                           id)))
                 (linked-from
                  (mapcar #'car
                          (org-roam-db-query
                           [:select [links:source] :from links
                            :inner-join tags :on (= links:source tags:node_id)
                            :where (= links:dest $s1)
                            :and   (= links:type "id")
                            :and   (= tags:tag "public")]
                           id))))
            (message "  [diag] step 4: encoding json")
            (let ((doc `(("id"          . ,id)
                         ("title"       . ,(org-roam-node-title node))
                         ("file"        . ,file)
                         ("point"       . ,(org-roam-node-point node))
                         ("level"       . ,(org-roam-node-level node))
                         ("tags"        . ,(apply #'vector (org-roam-node-tags node)))
                         ("aliases"     . ,(apply #'vector (org-roam-node-aliases node)))
                         ("links_to"    . ,(apply #'vector links-to))
                         ("linked_from" . ,(apply #'vector linked-from))
                         ("ast"         . ,serialized))))
              (message "  [diag] step 5: writing %s" out-path)
              (make-directory shard-dir t)
              (with-temp-file out-path
                (insert (json-encode doc)))
              (message "  [diag] step 6: done")))))
    (error
     (message "org-roam-export: ERROR on node %s: %S"
              (org-roam-node-id node) err))))

(defun org-roam-export--write-manifest (nodes output-dir)
  "Write manifest.json to OUTPUT-DIR with DB-level fields for all NODES.
The manifest contains id, title, file, tags, aliases, level for each node.
No AST is included in the manifest."
  (let* ((manifest-path (expand-file-name "manifest.json" output-dir))
         (entries
          (apply #'vector
                 (mapcar
                  (lambda (node)
                    `(("id"      . ,(org-roam-node-id node))
                      ("title"   . ,(org-roam-node-title node))
                      ("file"    . ,(org-roam-node-file node))
                      ("tags"    . ,(apply #'vector (org-roam-node-tags node)))
                      ("aliases" . ,(apply #'vector (org-roam-node-aliases node)))
                      ("level"   . ,(org-roam-node-level node))))
                  nodes))))
    (with-temp-file manifest-path
      (insert (json-encode entries)))
    (message "org-roam-export: manifest written to %s (%d entries)"
             manifest-path (length nodes))))

(defun org-roam-export--unique-nodes (nodes)
  "Return NODES deduplicated by ID, keeping the first occurrence of each.
org-roam-node-list includes one entry per alias, all sharing the same ID.
This reduces the list to one canonical node per unique ID."
  (let ((seen (make-hash-table :test 'equal))
        (result nil))
    (dolist (node nodes)
      (let ((id (org-roam-node-id node)))
        (unless (gethash id seen)
          (puthash id t seen)
          (push node result))))
    (nreverse result)))

(defun org-roam-export-to-json (output-dir)
  "Export entire org-roam database to JSON in OUTPUT-DIR.
Only nodes tagged \"public\" are exported.  Creates OUTPUT-DIR if it does
not exist. Writes one <id>.json file per node into subdirectories sharded
by the first two characters of the node ID, plus manifest.json at the
output root.
Returns the total number of nodes exported."
  (make-directory output-dir t)
  (let* ((nodes (cl-remove-if-not
                 (lambda (node) (member "public" (org-roam-node-tags node)))
                 (org-roam-export--unique-nodes (org-roam-node-list))))
         (total (length nodes))
         (count 0))
    (message "org-roam-export: exporting %d nodes..." total)
    (dolist (node nodes)
      (message "org-roam-export: [diag] node %s file %s"
               (org-roam-node-id node) (org-roam-node-file node))
      (org-roam-export--node-to-json node output-dir)
      (cl-incf count)
      (when (= (% count 10) 0)
        (garbage-collect))
      (when (= (% count 50) 0)
        (message "org-roam-export: %d/%d nodes..." count total)))
    (org-roam-export--write-manifest nodes output-dir)
    (message "org-roam-export: done. %d nodes -> %s" count output-dir)
    count))

(defun org-roam-export--batch-main ()
  "Entry point for batch-mode export. Reads configuration from environment.

Required environment variables:
  ORG_ROAM_DIR           -- path to org-roam notes directory
  ORG_ROAM_EXPORT_OUTPUT -- path to output directory for JSON files

Optional environment variables:
  ORG_ROAM_DB            -- path to org-roam.db
                            (default: ~/.emacs.d/org-roam.db)

Example invocation:
  ORG_ROAM_DIR=\"/path/to/notes\" \\
  ORG_ROAM_DB=\"$HOME/.emacs.d/org-roam.db\" \\
  ORG_ROAM_EXPORT_OUTPUT=\"/tmp/roam-export\" \\
  emacs -Q --batch --load /path/to/org-roam-export.el"
  (setq max-lisp-eval-depth 10000)
  (setq max-specpdl-size 10000)
  (setq print-circle t)  ; Prevent format "%S" from looping on circular objects
  ;; Keep GC aggressive in batch mode to prevent OOM on large note sets.
  ;; Emacs raises gc-cons-threshold at startup for performance; we lower it
  ;; here so collection runs frequently during the export loop.
  (setq gc-cons-threshold (* 32 1024 1024))
  (let* ((roam-dir   (or (getenv "ORG_ROAM_DIR")
                         (error "ORG_ROAM_DIR env var is required")))
         (roam-db    (or (getenv "ORG_ROAM_DB")
                         (expand-file-name "org-roam.db" user-emacs-directory)))
         (output-dir (or (getenv "ORG_ROAM_EXPORT_OUTPUT")
                         (error "ORG_ROAM_EXPORT_OUTPUT env var is required"))))
    (message "org-roam-export: roam-dir=%s" roam-dir)
    (message "org-roam-export: roam-db=%s" roam-db)
    (message "org-roam-export: output-dir=%s" output-dir)
    (require 'org-roam)
    (setq org-roam-directory roam-dir)
    (setq org-roam-db-location roam-db)
    (org-roam-export-to-json output-dir)))

;;; Batch entry point — only runs when loaded non-interactively (emacs --batch)
(when noninteractive
  (org-roam-export--batch-main))

;;; org-roam-export.el ends here
