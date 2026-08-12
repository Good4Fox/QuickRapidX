import { invoke } from '@tauri-apps/api/core';

/**
 * Поведение окна и запуска.
 *
 * Правда лежит в ядре: автозапуск — записью в реестре, остальное — в
 * `config.json`. Здесь только зеркало, и оно перечитывается при каждом входе в
 * настройки.
 *
 * Прежде состояние держалось в localStorage, а это отдельная память, которая
 * ничего не знает о реестре. Стоило убрать запись автозапуска через диспетчер
 * задач — и флажок продолжал показывать «включено» сколько угодно долго.
 */
type Prefs = {
	autostart: boolean;
	start_minimized: boolean;
	close_to_tray: boolean;
	always_on_top: boolean;
	remember_geometry: boolean;
	remember_view: boolean;
};

const EMPTY: Prefs = {
	autostart: false,
	start_minimized: false,
	close_to_tray: true,
	always_on_top: false,
	remember_geometry: false,
	remember_view: false
};

class WindowPrefs {
	autostart = $state(false);
	startMinimized = $state(false);
	closeToTray = $state(true);
	alwaysOnTop = $state(false);
	rememberGeometry = $state(false);
	/** Возвращаться на тот же раздел после показа окна. */
	rememberView = $state(false);

	/** Идёт запись — на это время флажки не отвечают на нажатия. */
	busy = $state(false);

	/** Перечитывает всё у ядра. */
	async load() {
		try {
			this.take(await invoke<Prefs>('window_prefs_get'));
		} catch (e) {
			console.error('не удалось прочитать настройки окна:', e);
		}
	}

	private take(prefs: Prefs) {
		this.autostart = prefs.autostart;
		this.startMinimized = prefs.start_minimized;
		this.closeToTray = prefs.close_to_tray;
		this.alwaysOnTop = prefs.always_on_top;
		this.rememberGeometry = prefs.remember_geometry;
		this.rememberView = prefs.remember_view;
	}

	/**
	 * Общий путь для всех переключателей.
	 *
	 * После записи состояние перечитывается у ядра, а не берётся из нажатия:
	 * команда может отказать или поправить соседнее значение, и тогда флажок
	 * обязан показать то, что вышло на самом деле.
	 */
	private async change(command: string, on: boolean) {
		if (this.busy) return;
		this.busy = true;

		try {
			await invoke(command, { on });
		} catch (e) {
			console.error(`${command} не выполнена:`, e);
		}

		await this.load();
		this.busy = false;
	}

	setAutostart(on: boolean) {
		return this.change('window_prefs_set_autostart', on);
	}

	setStartMinimized(on: boolean) {
		return this.change('window_prefs_set_start_minimized', on);
	}

	setCloseToTray(on: boolean) {
		return this.change('window_prefs_set_close_to_tray', on);
	}

	setAlwaysOnTop(on: boolean) {
		return this.change('window_prefs_set_always_on_top', on);
	}

	setRememberGeometry(on: boolean) {
		return this.change('window_prefs_set_remember_geometry', on);
	}

	setRememberView(on: boolean) {
		return this.change('window_prefs_set_remember_view', on);
	}

	reset() {
		this.take(EMPTY);
	}
}

export const windowPrefs = new WindowPrefs();
