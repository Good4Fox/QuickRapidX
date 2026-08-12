import { listen } from '@tauri-apps/api/event';

/**
 * Плавное появление содержимого при разворачивании окна.
 *
 * Само сворачивание и разворачивание анимирует Windows — ядро возвращает окну
 * биты стиля, по которым система это делает (см. `window_fx.rs`). Здесь только
 * доводка: после того как окно вернулось, содержимое проявляется, а не
 * возникает рывком в конце системной анимации.
 */

/** Класс, на котором держится анимация появления. */
const RESTORING = 'app_restoring';

/** Должно совпадать с длительностью в window-fx.css. */
const DURATION = 260;

export function setupWindowFx(): () => void {
	let alive = true;
	let timer: ReturnType<typeof setTimeout> | undefined;

	const play = () => {
		const app = document.getElementById('app');
		if (!app) return;

		// Снимаем и ставим заново: иначе повторное разворачивание не
		// перезапустит анимацию, класс ведь уже висит.
		app.classList.remove(RESTORING);
		void app.offsetWidth;
		app.classList.add(RESTORING);

		clearTimeout(timer);
		timer = setTimeout(() => app.classList.remove(RESTORING), DURATION);
	};

	const sub = listen('window:restored', () => {
		if (alive) play();
	});

	return () => {
		alive = false;
		clearTimeout(timer);
		sub.then((unlisten) => unlisten());
	};
}
