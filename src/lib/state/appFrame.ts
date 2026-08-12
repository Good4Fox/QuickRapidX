/**
 * Рамка окна.
 *
 * Окно создаётся без системных декораций, скругление и границу рисует #app.
 * В развёрнутом состоянии их надо убирать: скруглённые углы на весь экран
 * оставляют щели, сквозь которые видно рабочий стол.
 */

const BORDER = '1px solid rgba(255, 255, 255, 0.2)';
const RADIUS = '10px';

/** `true` — окно в обычном состоянии (рамка есть), `false` — развёрнуто. */
export function setAppFrame(framed: boolean): void {
	const app = document.getElementById('app');
	if (!app) return;

	app.style.borderRadius = framed ? RADIUS : '0px';
	app.style.border = framed ? BORDER : 'none';
}
