import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export function formatRelativeTime(iso: string) {
  const timestamp = new Date(iso).getTime();
  if (Number.isNaN(timestamp)) return iso;
  const diffMinutes = Math.max(0, Math.floor((Date.now() - timestamp) / 60_000));
  if (diffMinutes < 1) return "just now";
  if (diffMinutes < 60) return `${diffMinutes}m ago`;
  const diffHours = Math.floor(diffMinutes / 60);
  if (diffHours < 24) return `${diffHours}h ago`;
  return `${Math.floor(diffHours / 24)}d ago`;
}

export function titleFromPath(pathValue: string | null | undefined) {
  if (!pathValue) return "No workspace";
  const normalized = pathValue.replace(/\\/g, "/");
  const parts = normalized.split("/").filter(Boolean);
  return parts.at(-1) ?? pathValue;
}

export function pluckRecordValues<T>(record: Record<string, T>) {
  return Object.values(record);
}

export function sortByUpdatedAtDescending<T extends { updated_at: string }>(items: T[]) {
  return [...items].sort((left, right) => right.updated_at.localeCompare(left.updated_at));
}
