export type NavigateToFn = (path: string, line: number, column: number) => void;

let navigateImpl: NavigateToFn | null = null;

export function registerNavigateTo(fn: NavigateToFn): void {
  navigateImpl = fn;
}

export function navigateTo(path: string, line: number, column: number): void {
  navigateImpl?.(path, line, column);
}
