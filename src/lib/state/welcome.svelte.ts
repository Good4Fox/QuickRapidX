/**
 * Приветственный мастер: показывать его или нет.
 *
 * Отметка о том, что мастер пройден, лежит в хранилище окна под тем же ключом,
 * которым пользовался оригинал. Прежде её ставила и читала сама разметка — а
 * значит, посмотреть на мастер второй раз было нельзя ничем, кроме как вычистив
 * хранилище целиком, вместе с языком, темой и памятью об окне.
 *
 * Здесь это собрано в одно место, и появляется то, чего не хватало: способ
 * позвать мастер заново.
 */
const KEY = 'UserInteraction';

class Welcome {
	/** Показывать ли мастер прямо сейчас. */
	show = $state(false);

	/** Читает отметку при запуске окна. */
	restore() {
		this.show = !this.passed();
	}

	/** Пройден ли мастер. Вне окна хранилища нет — считаем, что пройден. */
	passed(): boolean {
		if (typeof localStorage === 'undefined') return true;

		return !!localStorage.getItem(KEY);
	}

	/** Позвать мастер заново — из настроек. */
	again() {
		if (typeof localStorage !== 'undefined') localStorage.removeItem(KEY);

		this.show = true;
	}

	/** Мастер пройден: больше не показывать. */
	done() {
		if (typeof localStorage !== 'undefined') localStorage.setItem(KEY, 'true');

		this.show = false;
	}
}

export const welcome = new Welcome();

/*
 * Чем человек пользуется — то, что он отметил на шаге выбора.
 *
 * Прежде эти галочки жили внутри разметки мастера и не читались нигде: человек
 * честно отмечал «Игры», а программа делала вид, что не заметила. Вопрос,
 * который не слушает ответ, хуже отсутствующего вопроса.
 *
 * Хранится здесь же, в хранилище окна, а не в настройках ядра: это не то, чем
 * ядро распоряжается, а подсказка интерфейсу, с какого раздела открыться.
 */
const USES_KEY = 'UserUses';

export type Use = 'fun' | 'games' | 'school' | 'making' | 'work' | 'family';

export function saveUses(list: Use[]) {
	if (typeof localStorage === 'undefined') return;

	localStorage.setItem(USES_KEY, list.join(','));
}

export function loadUses(): Use[] {
	if (typeof localStorage === 'undefined') return [];

	const raw = localStorage.getItem(USES_KEY);

	return raw ? (raw.split(',').filter(Boolean) as Use[]) : [];
}
