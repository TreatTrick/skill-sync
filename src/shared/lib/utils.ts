import { clsx, type ClassValue } from 'clsx'
import { twMerge } from 'tailwind-merge'

export const cn = (...inputs: ClassValue[]): string => twMerge(clsx(inputs))

// Type helpers used by shadcn-svelte components for ref forwarding and child
// snippet stripping.
export type WithElementRef<T, U extends HTMLElement = HTMLElement> = T & {
  ref?: U | null
}
// shadcn-svelte nova components reference these intermediate types directly, so export them
export type WithoutChildren<T> = Omit<T, 'children'>
export type WithoutChild<T> = Omit<T, 'child'>
export type WithoutChildrenOrChild<T> = WithoutChildren<WithoutChild<T>>
