// Count source lines in rust/ and design/.
//
// Run from the repository root with `deno task loc`.

const SOURCE_EXTENSIONS = [
  "c", "cc", "cpp", "css", "h", "hpp", "js", "jsx", "m", "mm", "py", "rs", "sh", "ts", "tsx", "zig",
];

const SKIPPED_DIRECTORIES = [
  ".git",
  ".zig-cache",
  "node_modules",
  "target",
  "test-videos",
  "vendor",
  "zig-out",
  "zig-pkg",
];

const LARGE_FILE_THRESHOLD = 1000;

interface Count {
  files: number;
  lines: number;
  nonBlank: number;
  largeFiles: string[];
}

const root = decodeURIComponent(new URL("./", import.meta.url).pathname).replace(/\/$/, "");
const counts = new Map<string, Count>();

try {
  for (const directory of ["rust", "design"]) {
    await countDirectory(`${root}/${directory}`, counts);
  }
} catch (error) {
  console.error(`Could not count ${root}: ${error}`);
  Deno.exit(1);
}

console.log(`Source lines in rust/ and design/ under ${root}\n`);
printCounts(root, counts);

function printCounts(root: string, counts: Map<string, Count>) {
  const row = (language: string, files: string, lines: string, nonBlank: string) =>
    console.log(`${language.padEnd(10)} ${files.padStart(8)} ${lines.padStart(12)} ${nonBlank.padStart(12)}`);
  const divider = () => console.log(`${"-".repeat(10)} ${"-".repeat(8)} ${"-".repeat(12)} ${"-".repeat(12)}`);
  const extensions = [...counts.keys()].sort();

  row("Language", "Files", "Lines", "Non-blank");
  divider();

  const total: Count = { files: 0, lines: 0, nonBlank: 0, largeFiles: [] };
  let totalLargeFiles = 0;
  for (const extension of extensions) {
    const count = counts.get(extension)!;
    row(extension, String(count.files), String(count.lines), String(count.nonBlank));
    total.files += count.files;
    total.lines += count.lines;
    total.nonBlank += count.nonBlank;
    totalLargeFiles += count.largeFiles.length;
  }

  divider();
  row("Total", String(total.files), String(total.lines), String(total.nonBlank));
  console.log();
  console.log(`Files with more than ${LARGE_FILE_THRESHOLD} lines:`);
  if (totalLargeFiles === 0) {
    console.log("  (none)");
  } else {
    for (const extension of extensions) {
      for (const path of counts.get(extension)!.largeFiles) {
        if (path.startsWith(`${root}/`)) {
          console.log(`  ${path.slice(root.length + 1)}`);
        } else {
          console.log(`  ${path}`);
        }
      }
    }
  }
}

async function countDirectory(directory: string, counts: Map<string, Count>) {
  for await (const entry of Deno.readDir(directory)) {
    const path = `${directory}/${entry.name}`;

    if (entry.isDirectory) {
      if (!SKIPPED_DIRECTORIES.includes(entry.name)) {
        await countDirectory(path, counts);
      }
      continue;
    }

    if (!entry.isFile) {
      continue;
    }

    // 与 Rust 的 Path::extension 一致：以点开头的文件名（如 .gitignore）没有扩展名。
    const dot = entry.name.lastIndexOf(".");
    if (dot <= 0) {
      continue;
    }
    const extension = entry.name.slice(dot + 1).toLowerCase();
    if (!SOURCE_EXTENSIONS.includes(extension)) {
      continue;
    }
    const language = entry.name.endsWith(".test.rs") ? "test.rs" : extension;

    const source = new TextDecoder().decode(await Deno.readFile(path));
    const lines = sourceLines(source);
    let count = counts.get(language);
    if (count === undefined) {
      count = { files: 0, lines: 0, nonBlank: 0, largeFiles: [] };
      counts.set(language, count);
    }
    count.files += 1;
    count.lines += lines.length;
    if (lines.length > LARGE_FILE_THRESHOLD) {
      count.largeFiles.push(path);
    }
    count.nonBlank += lines.filter((line) => line.trim() !== "").length;
  }
}

// 与 Rust 的 str::lines 一致：末尾换行不产生空行，行尾的 \r 被去掉。
function sourceLines(source: string): string[] {
  if (source === "") {
    return [];
  }
  const lines = source.split("\n").map((line) => line.replace(/\r$/, ""));
  if (source.endsWith("\n")) {
    lines.pop();
  }
  return lines;
}
