/// <reference types="vite/client" />
import html from '../index.html?raw';
import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { readThemePreference, THEME_STORAGE_KEY, useThemePreference } from './theme';

let systemDark = false;
let listeners: Set<EventListener>;
let media: MediaQueryList;

function changeSystem(dark: boolean) {
  systemDark = dark;
  act(() => {
    for (const listener of listeners) listener(new Event('change'));
  });
}

beforeEach(() => {
  localStorage.clear();
  document.documentElement.removeAttribute('data-theme');
  document.documentElement.removeAttribute('style');
  systemDark = false;
  listeners = new Set();
  media = {
    get matches() {
      return systemDark;
    },
    addEventListener: (_type: string, listener: EventListener) => listeners.add(listener),
    removeEventListener: (_type: string, listener: EventListener) => listeners.delete(listener),
  } as unknown as MediaQueryList;
  vi.stubGlobal(
    'matchMedia',
    vi.fn(() => media),
  );
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  localStorage.clear();
});

describe('appearance preference', () => {
  it('defaults to System, follows live changes and removes the subscription on unmount', () => {
    const { result, unmount } = renderHook(useThemePreference);
    expect(result.current[0]).toBe('system');
    expect(document.documentElement.dataset.theme).toBe('light');
    changeSystem(true);
    expect(document.documentElement.dataset.theme).toBe('dark');
    expect(document.documentElement.style.colorScheme).toBe('dark');
    expect(listeners.size).toBe(1);
    unmount();
    expect(listeners.size).toBe(0);
  });

  it('restores an explicit preference and ignores system changes until System is selected', () => {
    localStorage.setItem(THEME_STORAGE_KEY, 'light');
    systemDark = true;
    const { result } = renderHook(useThemePreference);
    expect(result.current[0]).toBe('light');
    expect(document.documentElement.dataset.theme).toBe('light');
    expect(listeners.size).toBe(0);
    changeSystem(false);
    changeSystem(true);
    expect(document.documentElement.dataset.theme).toBe('light');
    act(() => result.current[1]('system'));
    expect(localStorage.getItem(THEME_STORAGE_KEY)).toBe('system');
    expect(document.documentElement.dataset.theme).toBe('dark');
    changeSystem(false);
    expect(document.documentElement.dataset.theme).toBe('light');
  });

  it('persists a dark selection across remounts and releases the system listener', () => {
    const first = renderHook(useThemePreference);
    act(() => first.result.current[1]('dark'));
    expect(localStorage.getItem(THEME_STORAGE_KEY)).toBe('dark');
    expect(document.documentElement.dataset.theme).toBe('dark');
    expect(listeners.size).toBe(0);
    first.unmount();
    const second = renderHook(useThemePreference);
    expect(second.result.current[0]).toBe('dark');
    expect(document.documentElement.dataset.theme).toBe('dark');
  });

  it('ignores corrupt saved values and works when storage reads and writes are denied', () => {
    localStorage.setItem(THEME_STORAGE_KEY, 'unexpected');
    expect(readThemePreference()).toBe('system');
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('denied');
    });
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('denied');
    });
    systemDark = true;
    const { result } = renderHook(useThemePreference);
    expect(document.documentElement.dataset.theme).toBe('dark');
    act(() => result.current[1]('light'));
    expect(result.current[0]).toBe('light');
    expect(document.documentElement.dataset.theme).toBe('light');
  });

  it('still supports explicit selection when system preference lookup is unavailable', () => {
    vi.stubGlobal('matchMedia', () => {
      throw new Error('unavailable');
    });
    const { result } = renderHook(useThemePreference);
    expect(document.documentElement.dataset.theme).toBe('light');
    act(() => result.current[1]('dark'));
    expect(document.documentElement.dataset.theme).toBe('dark');
  });

  it('preloads the saved or system theme before the application module and tolerates denied storage', () => {
    const parsed = new DOMParser().parseFromString(html, 'text/html');
    const script = parsed.head.querySelector('script:not([src])')?.textContent;
    expect(script).toBeTruthy();
    expect(html.indexOf('<script>')).toBeLessThan(html.indexOf('src="/src/main.tsx"'));
    const preload = new Function('localStorage', 'matchMedia', 'document', script!);
    for (const [saved, dark, expected] of [
      ['dark', false, 'dark'],
      ['light', true, 'light'],
      ['system', true, 'dark'],
      ['corrupt', false, 'light'],
    ] as const) {
      preload({ getItem: () => saved }, () => ({ matches: dark }), document);
      expect(document.documentElement.dataset.theme).toBe(expected);
      expect(document.documentElement.style.colorScheme).toBe(expected);
    }
    preload(
      {
        getItem() {
          throw new Error('denied');
        },
      },
      () => ({ matches: true }),
      document,
    );
    expect(document.documentElement.dataset.theme).toBe('dark');
    renderHook(useThemePreference);
    expect(document.documentElement.style.backgroundColor).toBe('');
  });
});
