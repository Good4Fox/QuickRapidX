import { invoke } from '@tauri-apps/api/core';

/**
 * Корзина загрузок.
 *
 * Пункт «Скачать» задуман как итоговое место: человек отмечает нужное плюсиком
 * в каталоге и витрине, а потом забирает всё разом. Ставить при этом ничего не
 * надо — установщики складываются в «Загрузки/QuickRapidX», откуда их можно
 * унести на другую машину.
 *
 * Отмеченное переживает перезапуск: набирать список заново после случайного
 * закрытия окна незачем.
 */
export type BasketItem = {
	/** Устойчивый ключ: идентификатор пакета или ссылка. */
	key: string;
	name: string;
	/** Идентификатор winget. Пусто — качаем по ссылке. */
	packageId: string;
	url: string;
};

type Outcome = { name: string; ok: boolean; message: string };

const STORAGE_KEY = 'AppDownloadBasket';

class Basket {
	items = $state<BasketItem[]>([]);

	/** Что качается прямо сейчас. Пусто — загрузка не идёт. */
	current = $state<string | null>(null);

	/** Итоги последней загрузки. */
	report = $state<Outcome[]>([]);

	restore() {
		try {
			const raw = localStorage.getItem(STORAGE_KEY);
			if (raw) this.items = JSON.parse(raw);
		} catch {
			// Испорченное хранилище не повод ронять экран
			this.items = [];
		}
	}

	private keep() {
		localStorage.setItem(STORAGE_KEY, JSON.stringify(this.items));
	}

	has(key: string): boolean {
		return this.items.some((item) => item.key === key);
	}

	toggle(item: BasketItem) {
		// Качать нечем — класть в корзину незачем
		if (!item.packageId && !item.url) return;

		this.items = this.has(item.key)
			? this.items.filter((each) => each.key !== item.key)
			: [...this.items, item];

		this.keep();
	}

	clear() {
		this.items = [];
		this.keep();
	}

	/**
	 * Забирает всё отмеченное.
	 *
	 * По очереди, а не разом: параллельные загрузки делят канал и мешают друг
	 * другу, а отчёт по каждой позиции нужен построчно.
	 */
	async download(): Promise<Outcome[]> {
		if (this.current) return [];

		this.report = [];
		const done: Outcome[] = [];

		for (const item of this.items) {
			this.current = item.key;

			const outcome = item.packageId
				? await invoke<Outcome>('download_package', { id: item.packageId, name: item.name })
				: await invoke<Outcome>('download_link', { url: item.url, name: item.name });

			done.push(outcome);
		}

		this.current = null;
		this.report = done;

		// Успешно скачанное убираем: оставлять его значит качать заново при
		// следующем нажатии
		const failed = new Set(done.filter((outcome) => !outcome.ok).map((outcome) => outcome.name));
		this.items = this.items.filter((item) => failed.has(item.name));
		this.keep();

		return done;
	}
}

export const basket = new Basket();
