import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

import gold from '$lib/App/img/appicons/gold.png';
import star from '$lib/App/img/appicons/star.png';
import frost from '$lib/App/img/appicons/frost.png';
import mint from '$lib/App/img/appicons/mint.png';
import ember from '$lib/App/img/appicons/ember.png';
import orchid from '$lib/App/img/appicons/orchid.png';
import mono from '$lib/App/img/appicons/mono.png';

/**
 * Значок приложения.
 *
 * Хранит выбор ядро — в `config.json` рядом с прочими настройками, а не
 * localStorage: значок ставится на окно и в трей ещё до того, как интерфейс
 * загрузится, поэтому выбор нужен именно там.
 *
 * Здесь только картинки для предпросмотра. Файлы те же самые, что вшиты в
 * ядро, — одна копия на оба применения.
 */
export const ICON_PREVIEWS: Record<string, string> = {
	gold,
	star,
	frost,
	mint,
	ember,
	orchid,
	mono
};

/** Порядок в списке. Первый — тот, с которым программа ставится. */
export const ICON_ORDER = ['gold', 'star', 'frost', 'mint', 'ember', 'orchid', 'mono'] as const;

class AppIcon {
	current = $state('gold');

	/** Правка ярлыков трогает файлы вне приложения — только по просьбе. */
	withShortcuts = $state(false);

	private loaded = false;

	async load() {
		if (this.loaded) return;
		this.loaded = true;

		try {
			this.current = await invoke<string>('app_icon_get');
		} catch (e) {
			console.error('не удалось прочитать выбранный значок:', e);
		}

		this.mark();
	}

	/**
	 * Окон три, а настройки открыты в одном. Ядро рассылает выбор всем — иначе
	 * знак в полосе заголовка трея остался бы прежним до перезапуска.
	 */
	attach() {
		const sub = listen<string>('app:icon-changed', (event) => {
			this.current = event.payload;
			this.mark();
		});

		return () => {
			sub.then((unlisten) => unlisten());
		};
	}

	/**
	 * Подставляет знак в оформление.
	 *
	 * Одним токеном на все места, где он рисуется: полоса заголовка главного
	 * окна и трея, экран загрузки, карточка в «О программе».
	 */
	private mark() {
		const src = ICON_PREVIEWS[this.current] ?? ICON_PREVIEWS.gold;
		document.documentElement.style.setProperty('--app-mark', `url(${src})`);
	}

	/** Ставит значок. Возвращает false, если ядро отказало. */
	async set(id: string): Promise<boolean> {
		const previous = this.current;
		this.current = id;

		this.mark();

		try {
			await invoke('app_icon_set', { id, shortcuts: this.withShortcuts });
			return true;
		} catch (e) {
			console.error('не удалось поставить значок:', e);
			this.current = previous;
			this.mark();
			return false;
		}
	}
}

export const appIcon = new AppIcon();
