import { clsx, type ClassValue } from 'clsx'
import { twMerge } from 'tailwind-merge'

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

// Explicit locale: the server and the browser must format identically or the
// hydrated markup will not match the server-rendered text.
const LOCALE = 'en-US'

export function formatStroops(stroops: number | string): string {
  const n = typeof stroops === 'string' ? parseFloat(stroops) : stroops
  // A non-numeric string parses to NaN; render an em dash rather than 'NaN str'.
  if (!Number.isFinite(n)) return '—'
  if (n >= 10_000_000) return `${(n / 10_000_000).toFixed(2)} XLM`
  if (n >= 1_000) return `${n.toLocaleString(LOCALE)} str`
  return `${n} str`
}

export function formatNumber(n: number): string {
  return n.toLocaleString()
}

export function pctColor(pct: number | null): string {
  if (pct === null) return 'text-text-secondary'
  if (pct > 10)  return 'text-accent-red'
  if (pct > 0)   return 'text-accent-yellow'
  if (pct < -10) return 'text-accent-green'
  if (pct < 0)   return 'text-accent-cyan'
  return 'text-text-secondary'
}

export function pctArrow(pct: number | null): string {
  if (pct === null) return '—'
  if (pct > 0) return `▲ +${pct.toFixed(1)}%`
  if (pct < 0) return `▼ ${pct.toFixed(1)}%`
  return `→ 0.0%`
}

export function congestionColor(status: string): string {
  switch (status) {
    case 'Congested': return 'text-accent-red'
    case 'Rising':    return 'text-accent-yellow'
    case 'Declining': return 'text-accent-cyan'
    default:          return 'text-accent-green'
  }
}

export function congestionBg(status: string): string {
  switch (status) {
    case 'Congested': return 'bg-red-950/30 border-accent-red/30'
    case 'Rising':    return 'bg-yellow-950/30 border-accent-yellow/30'
    case 'Declining': return 'bg-cyan-950/30 border-accent-cyan/30'
    default:          return 'bg-accent-dim border-accent-green/30'
  }
}

export function timeAgo(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime()
  const s = Math.floor(diff / 1000)
  if (s < 60)  return `${s}s ago`
  if (s < 3600) return `${Math.floor(s / 60)}m ago`
  return `${Math.floor(s / 3600)}h ago`
}

export function formatTime(iso: string): string {
  return new Date(iso).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' })
}

export function formatStroop(value: number): string {
  return `${value.toLocaleString()} stroops`
}

export function formatTimestamp(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime()
  const seconds = Math.floor(diff / 1000)
  if (seconds < 60) return `${seconds} secs ago`
  const minutes = Math.floor(seconds / 60)
  if (minutes < 60) return `${minutes} mins ago`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours} hours ago`
  const days = Math.floor(hours / 24)
  return `${days} days ago`
}