/**
 * Уведомления приложения.
 *
 * Прошлая версия была заглушкой: хранила три поля (текст, цвет подложки и имя
 * класса иконки), тексты брала из таблицы на два сообщения, а создавать
 * уведомления было неоткуда — ни один экран их не добавлял.
 *
 * Здесь полноценная модель: уровень, заголовок, подробности, время и признак
 * прочтения. История переживает перезапуск и ограничена по длине.
 */

import { i18n } from '$lib/state/i18n.svelte';

/** Насколько событие важное. От уровня зависят цвет и значок. */
export type Level = 'info' | 'success' | 'warning' | 'error';

export type Notification = {
	id: string;
	level: Level;
	/** Короткая строка — то, что видно всегда. */
	title: string;
	/** Подробности: причина ошибки, путь, код. Необязательны. */
	text?: string;
	/** Когда произошло, миллисекунды эпохи. */
	at: number;
	read: boolean;
};

/** Что нужно, чтобы создать уведомление. */
export type NewNotification = {
	level?: Level;
	title: string;
	text?: string;
	/**
	 * Показывать всплывающим окошком. По умолчанию показываются все, кроме
	 * `info` — иначе фон шумит на каждое мелкое действие.
	 */
	toast?: boolean;
};

/**
 * Насколько подробно уведомлять.
 *
 * `all` — как было: всё попадает в историю и всплывает.
 * `quiet` — удачные действия только мелькают окошком и в историю не ложатся:
 *   очищать становится нечего, а подтверждение действия остаётся.
 * `alerts` — только предупреждения и ошибки, остальное молчит совсем.
 */
export type Mode = 'all' | 'quiet' | 'alerts' | 'custom';

/**
 * Правила: что показывать окошком, что оставлять в списке.
 *
 * По уровням, а не одним переключателем на всё: удачное завершение и ошибка —
 * события разного веса, и держать их вместе значит либо терять ошибки в шуме,
 * либо чистить список из-за мелочей.
 */
export type Rules = {
	toast: Record<Level, boolean>;
	keep: Record<Level, boolean>;
	/** Сколько окошко висит на экране, миллисекунды. */
	linger: number;
	/** Сколько записей держим в списке. */
	limit: number;
};

const LEVELS: Level[] = ['info', 'success', 'warning', 'error'];

/** Готовые наборы правил. «Своё» здесь нет: оно и есть правка вручную. */
const PRESETS: Record<Exclude<Mode, 'custom'>, Rules> = {
	all: {
		toast: { info: false, success: true, warning: true, error: true },
		keep: { info: true, success: true, warning: true, error: true },
		linger: 5000,
		limit: 50
	},
	quiet: {
		toast: { info: false, success: true, warning: true, error: true },
		keep: { info: false, success: false, warning: true, error: true },
		linger: 4000,
		limit: 50
	},
	alerts: {
		toast: { info: false, success: false, warning: true, error: true },
		keep: { info: false, success: false, warning: true, error: true },
		linger: 6000,
		limit: 50
	}
};

export function presetRules(mode: Exclude<Mode, 'custom'>): Rules {
	const source = PRESETS[mode];

	return {
		toast: { ...source.toast },
		keep: { ...source.keep },
		linger: source.linger,
		limit: source.limit
	};
}

const STORAGE_KEY = 'QuickRapidX.notifications';
const MODE_KEY = 'QuickRapidX.notifications.mode';
const RULES_KEY = 'QuickRapidX.notifications.rules';

function readMode(): Mode {
	const saved = localStorage.getItem(MODE_KEY);

	return saved === 'quiet' || saved === 'alerts' || saved === 'custom' ? saved : 'all';
}

function readRules(mode: Mode): Rules {
	const fallback = presetRules(mode === 'custom' ? 'all' : mode);

	try {
		const raw = localStorage.getItem(RULES_KEY);
		if (!raw) return fallback;

		const parsed = JSON.parse(raw) as Partial<Rules>;

		// Разбираем по полю: файл мог остаться от прежней версии, и доверять
		// ему целиком нельзя
		for (const level of LEVELS) {
			if (typeof parsed.toast?.[level] === 'boolean') fallback.toast[level] = parsed.toast[level];
			if (typeof parsed.keep?.[level] === 'boolean') fallback.keep[level] = parsed.keep[level];
		}

		if (typeof parsed.linger === 'number') fallback.linger = clamp(parsed.linger, 1500, 20000);
		if (typeof parsed.limit === 'number') fallback.limit = clamp(parsed.limit, 10, 300);

		return fallback;
	} catch {
		return fallback;
	}
}

function clamp(value: number, low: number, high: number): number {
	return Math.min(high, Math.max(low, Math.round(value)));
}

/** Сколько записей держим по умолчанию. Дальше вытесняются самые старые. */
const HISTORY_LIMIT = 50;

/** Сколько всплывающее окошко висит на экране по умолчанию. */
const TOAST_MS = 5000;

function read(): Notification[] {
	try {
		const raw = localStorage.getItem(STORAGE_KEY);
		if (!raw) return [];

		const parsed: unknown = JSON.parse(raw);
		if (!Array.isArray(parsed)) return [];

		// Записи могли быть сохранены прошлой версией — берём только пригодные
		return parsed.filter(
			(n): n is Notification =>
				!!n && typeof n === 'object' && typeof (n as Notification).title === 'string'
		);
	} catch {
		return [];
	}
}

class Notifications {
	items = $state<Notification[]>([]);

	/** Показанные прямо сейчас всплывающие окошки. */
	toasts = $state<Notification[]>([]);

	private timers = new Map<string, ReturnType<typeof setTimeout>>();

	/** Сколько непрочитанных — по этому числу колокольчик меняет вид. */
	get unread(): number {
		return this.items.filter((n) => !n.read).length;
	}

	get any(): boolean {
		return this.items.length > 0;
	}

	/** Выбранный набор правил. */
	mode = $state<Mode>('all');

	/** Сами правила. Набор их только заполняет. */
	rules = $state<Rules>(presetRules('all'));

	/** Поднимает историю с диска. Вызывать после монтирования. */
	restore() {
		this.items = read();
		this.mode = readMode();
		this.rules = readRules(this.mode);
	}

	/** Выбор готового набора: правила заполняются им целиком. */
	setMode(next: Mode) {
		this.mode = next;

		if (next !== 'custom') this.rules = presetRules(next);

		this.saveRules();
	}

	/** Правка вручную. Набор при этом становится своим. */
	tune(next: Partial<Rules>) {
		this.rules = { ...this.rules, ...next };
		this.mode = 'custom';
		this.saveRules();

		// Список мог оказаться длиннее нового предела
		if (this.items.length > this.rules.limit) {
			this.items = this.items.slice(0, this.rules.limit);
			this.persist();
		}
	}

	private saveRules() {
		try {
			localStorage.setItem(MODE_KEY, this.mode);
			localStorage.setItem(RULES_KEY, JSON.stringify($state.snapshot(this.rules)));
		} catch (e) {
			console.error('не удалось сохранить настройки уведомлений:', e);
		}
	}

	/**
	 * Добавляет уведомление и возвращает его.
	 *
	 * Возвращает `null`, если выбранный режим этому событию не даёт хода.
	 */
	push({ level = 'info', title, text, toast }: NewNotification): Notification | null {
		const shows = toast ?? this.rules.toast[level];
		const keeps = this.rules.keep[level];

		// Ни окошком, ни в списке — событию просто не давали хода
		if (!shows && !keeps) return null;

		const note: Notification = {
			id: crypto.randomUUID(),
			level,
			title,
			text,
			at: Date.now(),
			read: false
		};

		/*
		 * Окошко и список разделены намеренно.
		 *
		 * Удачное действие достаточно подтвердить на несколько секунд: именно
		 * накопившийся список и заставляет заходить туда и чистить его вручную.
		 * А ошибку, наоборот, надо суметь перечитать позже.
		 */
		if (keeps) {
			this.items.unshift(note);

			if (this.items.length > this.rules.limit) {
				this.items.length = this.rules.limit;
			}

			this.persist();
		}

		if (shows) this.showToast(note);

		return note;
	}

	markAllRead() {
		for (const note of this.items) {
			note.read = true;
		}
		this.persist();
	}

	dismiss(id: string) {
		this.items = this.items.filter((n) => n.id !== id);
		this.persist();
	}

	clear() {
		this.items = [];
		this.persist();
	}

	/** Убирает окошко с экрана, не трогая историю. */
	hideToast(id: string) {
		this.toasts = this.toasts.filter((n) => n.id !== id);

		const timer = this.timers.get(id);
		if (timer) {
			clearTimeout(timer);
			this.timers.delete(id);
		}
	}

	private showToast(note: Notification) {
		this.toasts.push(note);
		this.timers.set(
			note.id,
			setTimeout(() => this.hideToast(note.id), this.rules.linger || TOAST_MS)
		);
	}

	private persist() {
		try {
			localStorage.setItem(STORAGE_KEY, JSON.stringify(this.items));
		} catch (e) {
			// Переполненное хранилище не должно ронять интерфейс
			console.error('не удалось сохранить уведомления:', e);
		}
	}
}

export const notifications = new Notifications();

/** Короткая запись для мест, где важен только факт ошибки. */
export function notifyError(title: string, text?: string) {
	notifications.push({ level: 'error', title, text });
}

export function notifySuccess(title: string, text?: string) {
	notifications.push({ level: 'success', title, text });
}

/**
 * «только что», «5 мин назад», «вчера».
 *
 * Точное время в списке уведомлений почти никогда не нужно, а относительное
 * читается с одного взгляда.
 */
export function relativeTime(at: number, now = Date.now()): string {
	/*
	 * Формы слов отдаём системе, а не собираем сами.
	 *
	 * Прежде здесь лежали шесть русских фраз, и они показывались под каждой
	 * записью на любом языке. Заводить вместо них шесть ключей значило бы
	 * упереться в склонения: «2 минуты назад», но «5 минут назад», а в японском
	 * нужна и вовсе другая мерка. `Intl` знает это про каждый язык сам, а
	 * `numeric: 'auto'` вдобавок сам скажет «вчера», «yesterday» и 「昨日」.
	 */
	const when = new Intl.RelativeTimeFormat(i18n.current.tag, { numeric: 'auto' });
	const seconds = Math.max(0, Math.round((now - at) / 1000));

	if (seconds < 45) return when.format(0, 'second');
	if (seconds < 90) return when.format(-1, 'minute');

	const minutes = Math.round(seconds / 60);
	if (minutes < 60) return when.format(-minutes, 'minute');

	const hours = Math.round(minutes / 60);
	if (hours < 24) return when.format(-hours, 'hour');

	const days = Math.round(hours / 24);
	if (days < 7) return when.format(-days, 'day');

	// Дальше недели относительное время перестаёт что-либо говорить — дата
	// понятнее. Метка языка нужна и здесь: без неё формат берётся из системы,
	// а он с выбранным языком не связан
	return new Date(at).toLocaleDateString(i18n.current.tag);
}
