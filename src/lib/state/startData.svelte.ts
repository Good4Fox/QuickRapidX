import { invoke } from '@tauri-apps/api/core';
import { getVersion } from '@tauri-apps/api/app';

import { loadSystemInfo, type SystemInfo } from '$lib/state/systemInfo';

/**
 * Данные стартовой страницы.
 *
 * Собираются заранее — пока держится экран загрузки, — а не в момент показа
 * страницы. Иначе плитки успевают мигнуть прочерками: запросы уходят к ядру
 * уже после того, как приложение появилось на экране.
 */
class StartData {
	info = $state<SystemInfo | null>(null);
	version = $state('');
	binUsage = $state<number | null>(null);
	entries = $state<number | null>(null);

	/** Первый сбор завершён — можно показывать значения, а не заглушки. */
	ready = $state(false);

	private started = false;

	/**
	 * Собирает всё разом. Повторные вызовы игнорируются: страница может
	 * открываться много раз, а данные нужны один.
	 */
	async load() {
		if (this.started) return;
		this.started = true;
		await this.refresh();
		this.ready = true;
	}

	/** Перечитывает изменчивые значения: память, время работы, корзину. */
	async refresh() {
		await Promise.all([
			loadSystemInfo().then((value) => {
				if (value) this.info = value;
			}),

			getVersion()
				.then((v) => (this.version = v))
				.catch(() => undefined),

			invoke<number>('tray_basket_get_current_usage')
				.then((v) => (this.binUsage = v))
				.catch(() => undefined),

			invoke<string>('json_data_info')
				.then((raw) => {
					const lists: unknown[][] = JSON.parse(raw);
					this.entries = lists.reduce((sum, list) => sum + list.length, 0);
				})
				.catch(() => undefined)
		]);
	}
}

export const startData = new StartData();
