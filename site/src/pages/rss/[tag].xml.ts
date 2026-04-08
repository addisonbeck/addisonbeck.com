import rss from "@astrojs/rss";
import { readFileSync } from "fs";
import { resolve } from "path";

const SITE = "https://addisonbeck.com";
const INDEX_PATH = resolve(process.cwd(), "../rendered/index.json");

export function getStaticPaths() {
  const index = JSON.parse(readFileSync(INDEX_PATH, "utf-8"));

  const tags = new Set<string>();
  for (const node of index) {
    for (const tag of node.tags ?? []) {
      tags.add(tag);
    }
  }
  tags.add("all");

  return [...tags].map((tag) => ({ params: { tag } }));
}

export async function GET({ params }: { params: { tag: string } }) {
  const index = JSON.parse(readFileSync(INDEX_PATH, "utf-8"));
  const { tag } = params;

  const nodes =
    tag === "all"
      ? index
      : index.filter((n: { tags?: string[] }) => n.tags?.includes(tag));

  return rss({
    title: tag === "all" ? "addisonbeck.com" : `addisonbeck.com — ${tag}`,
    description:
      tag === "all"
        ? "All public notes from addisonbeck.com"
        : `Notes tagged "${tag}" from addisonbeck.com`,
    site: SITE,
    items: nodes.map(
      (node: {
        id: string;
        title: string;
        slug: string;
        last_modified: string;
      }) => ({
        title: node.title,
        pubDate: new Date(node.last_modified),
        link: `/${node.slug}`,
        content: readFileSync(
          resolve(process.cwd(), `../rendered/${node.id}.html`),
          "utf-8",
        ),
      }),
    ),
  });
}
