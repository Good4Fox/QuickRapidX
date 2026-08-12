import { languages } from '$lib/data/language';

export type LanguageKey = keyof typeof languages;

/**
 * Языки интерфейса.
 *
 * Раньше связка держалась на отображаемом имени: в localStorage лежала строка
 * «Русский», а словарь искался по ней через таблицу имя → ключ. Переименовать
 * язык в списке значило сбросить выбор у всех, кто его выбрал, а опечатка в
 * имени тихо откатывала интерфейс на английский. Теперь хранится ключ словаря,
 * а имя — просто подпись.
 */
/*
 * Подпись `note` намеренно на английском во всех трёх строках, а не переводится.
 *
 * Она нужна ровно в одном случае: человек промахнулся языком и смотрит на
 * список, которого не читает. Тогда единственная зацепка — знакомая латиница.
 * Перевод её на выбранный язык эту зацепку и убрал бы: японец, попавший в
 * русский интерфейс, увидел бы «Японский» кириллицей.
 */
export const LANGUAGES = [
	{ key: 'app_languages_en', tag: 'en', name: 'English', note: 'English' },
	{ key: 'app_languages_ru', tag: 'ru', name: 'Русский', note: 'Russian' },
	{ key: 'app_languages_jp', tag: 'ja', name: '日本語', note: 'Japanese' }
] as const satisfies readonly { key: LanguageKey; tag: string; name: string; note: string }[];

/** Словарь одного языка: плоский набор строк по ключам. */
export type Dictionary = (typeof languages)[LanguageKey][0];

const STORAGE_KEY = 'PlayerUserLanguages';

const DEFAULT: LanguageKey = 'app_languages_en';

/**
 * Язык, на котором говорит сама система.
 *
 * Спрашиваем движок, а не ядро: приветственный мастер показывается в первые
 * мгновения, и ждать ответа по каналу к ядру пришлось бы дольше, чем рисуется
 * первый кадр. WebView2 берёт этот список из настроек Windows, так что ответ
 * тот же самый.
 *
 * Берём весь список предпочтений, а не только первый: у человека может стоять
 * японский первым и русский вторым, и если японского у нас бы не было, второй
 * подошёл бы лучше английского.
 */
function fromSystem(): LanguageKey | null {
	if (typeof navigator === 'undefined') return null;

	const wanted = navigator.languages?.length ? navigator.languages : [navigator.language];

	for (const tag of wanted) {
		// Сравниваем только первую часть метки: `ru-RU`, `ru` и `ru-BY` — один
		// и тот же наш словарь
		const short = tag?.toLowerCase().split('-')[0];
		const found = LANGUAGES.find((language) => language.tag === short);

		if (found) return found.key;
	}

	return null;
}

/** Старый формат хранения — отображаемое имя. Понадобится один раз, при переходе. */
const LEGACY: Record<string, LanguageKey> = {
	English: 'app_languages_en',
	Русский: 'app_languages_ru',
	日本語: 'app_languages_jp'
};

function isKey(value: string | null): value is LanguageKey {
	return value !== null && value in languages;
}

/**
 * Текущий язык интерфейса.
 *
 * В прошлой версии переводы отдавались массивом, и вся страница оборачивалась
 * в `{#each LangGet as item, i}` — приложение рендерилось по разу на каждый
 * элемент массива, а лишние копии гасились через `{#if i === 0}`. Здесь словарь
 * отдаётся объектом, а реактивность даёт руна: `{t.home_text_1}` и всё.
 */
class I18n {
	key = $state<LanguageKey>(DEFAULT);

	/** Словарь текущего языка. */
	get t(): Dictionary {
		return languages[this.key][0];
	}

	/** Описание текущего языка: ключ, метка языка и подпись. */
	get current() {
		return LANGUAGES.find((language) => language.key === this.key) ?? LANGUAGES[0];
	}

	/** Отображаемое имя текущего языка. */
	get name(): string {
		return this.current.name;
	}

	/** Ставит язык по ключу словаря и запоминает выбор. */
	set(key: LanguageKey) {
		if (!(key in languages)) return;

		this.key = key;
		localStorage.setItem(STORAGE_KEY, key);
		this.mark();
	}

	/** Поднимает сохранённый выбор. Вызывать после монтирования. */
	restore() {
		const saved = localStorage.getItem(STORAGE_KEY);

		if (isKey(saved)) {
			this.key = saved;
		} else if (saved && saved in LEGACY) {
			// Перевод старого значения в новый формат — молча, один раз
			this.key = LEGACY[saved];
			localStorage.setItem(STORAGE_KEY, this.key);
		} else {
			/*
			 * Выбора ещё не было — берём язык системы.
			 *
			 * Прежде здесь безусловно вставал английский, и человек с русской
			 * или японской Windows встречал приветственный мастер на чужом
			 * языке. Свой выбор мы не запоминаем: он не выбор, а догадка, и
			 * если человек ткнёт в переключатель — запомнится уже настоящий.
			 */
			this.key = fromSystem() ?? DEFAULT;
		}

		this.mark();
	}

	/**
	 * Проставляет язык документу. От него зависят переносы, кавычки и то, какое
	 * начертание берёт браузер для общих знаков китайского и японского.
	 */
	private mark() {
		document.documentElement.lang = this.current.tag;
	}
}

export const i18n = new I18n();
