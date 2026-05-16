export function languageForPath(path: string): string {
  const lower = path.toLowerCase();
  if (lower.endsWith(".c")) return "c";
  if (lower.endsWith(".h")) return "cpp";
  if (lower.endsWith(".cpp") || lower.endsWith(".cc")) return "cpp";
  if (lower.endsWith(".ioc")) return "ini";
  if (lower.endsWith(".ld")) return "plaintext";
  return "plaintext";
}
