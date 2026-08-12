import type { Level } from '$lib/state/notifications.svelte';

/** Что капля умеет показывать. */
export type IslandKind = Level | 'progress';

export type IslandItem = {
	kind: IslandKind;
	title: string;
	text?: string;
	/** Для `progress` — доля выполнения, 0..100. */
	percent?: number;
};

/** Сколько висит сообщение, показанное через `show`. */
const SHOW_MS = 3200;

/**
 * Состояние «капли» в полосе заголовка.
 *
 * Два способа занять её:
 *   `show` — показать и убрать самому через несколько секунд (события);
 *   `pin`  — держать, пока не снимут (длящиеся процессы вроде загрузки).
 *
 * Закреплённое всегда важнее мимолётного: если идёт загрузка, всплывшее
 * уведомление её не перекроет.
 */
class Island {
	private transient = $state<IslandItem | null>(null);
	private pinned = $state<IslandItem | null>(null);

	private timer: ReturnType<typeof setTimeout> | undefined;

	/** Что показывать прямо сейчас. */
	get current(): IslandItem | null {
		return this.pinned ?? this.transient;
	}

	/** Показать и убрать через несколько секунд. */
	show(item: IslandItem) {
		this.transient = item;

		clearTimeout(this.timer);
		this.timer = setTimeout(() => {
			this.transient = null;
		}, SHOW_MS);
	}

	/** Держать, пока не снимут. */
	pin(item: IslandItem) {
		this.pinned = item;
	}

	unpin() {
		this.pinned = null;
	}
}

export const island = new Island();
