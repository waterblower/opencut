// Renders every page in pages/ to a standalone HTML file in ../pages/.
// Run with: deno task build

import type { Markup } from "./jsx/jsx-runtime.ts";

const pagesDirectory = new URL("./pages/", import.meta.url);
const distDirectory = new URL("../pages/", import.meta.url);
await Deno.mkdir(distDirectory, { recursive: true });

const pageFiles = [...Deno.readDirSync(pagesDirectory)]
  .filter((entry) => entry.isFile && entry.name.endsWith(".tsx"))
  .map((entry) => entry.name)
  .sort();

for (const fileName of pageFiles) {
  const name = fileName.replace(/\.tsx$/, "");
  const module = await import(new URL(fileName, pagesDirectory).href);
  const page: () => Markup = module.default;
  const html = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>${name}</title>
<style>body{margin:0}svg{display:block}</style>
</head>
<body>
${page().html}
</body>
</html>
`;
  // The output is static markup; JSX only runs here, at build time.
  if (/<script\b|\son[a-z]+=/i.test(html)) {
    throw new Error(`${name}: the built HTML must not contain JavaScript`);
  }
  await Deno.writeTextFile(new URL(`${name}.html`, distDirectory), html);
  console.log(`pages/${name}.html`);
}
