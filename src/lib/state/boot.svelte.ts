import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

import type { Dictionary } from '$lib/state/i18n.svelte';

/** Полезная нагрузка события `boot:progress` из Rust. */
export type BootProgress = {
	/** Номер шага, начиная с 1. */
	index: number;
	/** Сколько всего шагов в загрузке. */
	total: number;
	/** Машинный ключ шага: на нём и строится подпись. */
	key: string;
	/** Прогресс всей загрузки, 0..100. */
	percent: number;
};

/** Полезная нагрузка события `boot:failed`. */
export type BootFailure = {
	key: string;
	message: string;
};

/**
 * Состояние загрузки приложения.
 *
 * Значения приходят событиями из Rust — фронтенд ничего не выдумывает и не
 * крутит собственных таймеров. Если бэкенд встанет на шаге, полоса честно
 * замрёт на нём, а не доедет до ста процентов сама по себе.
 */
class BootState {
	percent = $state(0);

	/*
	 * Шаг хранится ключом, а не готовой подписью.
	 *
	 * Прежде подпись приходила из ядра русской строкой, и экран загрузки —
	 * второй, который человек видит при каждом запуске, — оставался русским на
	 * любом языке. Ключ ядро присылало всё это время рядом, просто им никто не
	 * пользовался.
	 */
	key = $state('init');
	index = $state(0);
	total = $state(0);
	ready = $state(false);
	error = $state<BootFailure | null>(null);

	/**
	 * Подписывается на события загрузки.
	 * Возвращает функцию отписки — вызывать при размонтировании окна.
	 *
	 * Сразу после подписки состояние ещё и опрашивается: события рассылаются
	 * один раз и без повтора, а окно могло не успеть подписаться до конца
	 * загрузки — тогда оно ждало бы `boot:ready`, которого уже не будет.
	 */
	async attach(): Promise<UnlistenFn> {
		const off = await Promise.all([
			listen<BootProgress>('boot:progress', ({ payload }) => {
				this.index = payload.index;
				this.total = payload.total;
				this.key = payload.key;
				this.percent = payload.percent;
			}),
			listen<null>('boot:ready', () => {
				this.percent = 100;
				this.ready = true;
			}),
			listen<BootFailure>('boot:failed', ({ payload }) => {
				this.error = payload;
			})
		]);

		// Догоняем пропущенное: если загрузка закончилась, пока подписка ещё
		// оформлялась, события уже не будет — спрашиваем ядро напрямую.
		if (!this.ready) {
			try {
				if (await invoke<boolean>('boot_status')) {
					this.percent = 100;
					this.ready = true;
				}
			} catch (e) {
				console.error('не удалось узнать состояние загрузки:', e);
			}
		}

		return () => {
			off.forEach((unlisten) => {
				unlisten();
			});
		};
	}
}

export const boot = new BootState();

/**
 * Подпись шага загрузки.
 *
 * Словарь передаётся доводом, а не берётся здесь: модуль не компонент, руны
 * ему не полагаются, и обращение к `i18n.t` изнутри не пересчиталось бы при
 * смене языка.
 *
 * Разбор по случаям, а не сборка имени ключа строкой: так каждая подпись
 * проверяется при сборке, и забытый в словаре шаг падает на `svelte-check`, а
 * не превращается в пустое место на экране.
 */
export function bootLabel(t: Dictionary, key: string): string {
	switch (key) {
		case 'paths':
			return t.boot_step_paths;
		case 'config':
			return t.boot_step_config;
		case 'system':
			return t.boot_step_system;
		case 'modules':
			return t.boot_step_modules;
		case 'interface':
			return t.boot_step_interface;
		default:
			return t.boot_init;
	}
}
