/**
 * Блокировка браузерных сочетаний клавиш и контекстного меню.
 *
 * Окно приложения — это WebView, и в нём по умолчанию живут все сочетания
 * браузера: перезагрузка, печать, поиск по странице, масштаб. В десктопном
 * приложении они не к месту. В режиме разработки блокировка отключена, чтобы
 * оставались доступны devtools.
 */

const DEV = import.meta.env.MODE === 'development';

/** Функциональные клавиши: F12, F3, F5, F7. */
const BLOCKED_KEYS = new Set(['F12', 'F3', 'F5', 'F7']);

/** Сочетания с модификаторами. Записаны так же, как их собирает `combination`. */
const BLOCKED_COMBINATIONS = new Set([
	'Ctrl+Shift+S', 'Ctrl+Shift+J', 'Ctrl+Shift+B', 'Alt+Shift+B',
	'Ctrl+D', 'Ctrl+Shift+D', 'Ctrl+Shift+E', 'Alt+D',
	'Ctrl+E', 'Alt+E', 'Ctrl+F', 'Alt+F',
	'Ctrl+G', 'Ctrl+Shift+G', 'Ctrl+H', 'Ctrl+Shift+I', 'Alt+Shift+I',
	'Ctrl+J', 'Ctrl+K', 'Ctrl+Shift+K', 'Ctrl+L', 'Ctrl+Shift+L',
	'Ctrl+M', 'Ctrl+Shift+M', 'Ctrl+N', 'Ctrl+Shift+N',
	'Ctrl+O', 'Ctrl+Shift+O', 'Ctrl+P', 'Ctrl+Shift+P',
	'Ctrl+R', 'Ctrl+Shift+R', 'Ctrl+S', 'Ctrl+T', 'Ctrl+Shift+T', 'Alt+Shift+T',
	'Ctrl+U', 'Ctrl+Shift+U', 'Ctrl+Shift+V', 'Ctrl+W', 'Ctrl+Shift+W',
	'Ctrl+Shift+Y',
	'Ctrl+0', 'Ctrl+1', 'Ctrl+2', 'Ctrl+3', 'Ctrl+4',
	'Ctrl+5', 'Ctrl+6', 'Ctrl+7', 'Ctrl+8', 'Ctrl+9',
	'Ctrl+ENTER', 'Ctrl+TAB', 'Ctrl+Shift+TAB',
	'Ctrl+\\', 'Ctrl+[', 'Ctrl+]',
	'Ctrl+Shift+DELETE',
	'Alt+ARROWLEFT', 'Alt+ARROWRIGHT', 'Alt+HOME',
	'Shift+ ', 'PAGEDOWN', 'Ctrl+PAGEDOWN', 'PAGEUP', 'Ctrl+PAGEUP', 'Shift+TAB'
]);

function combination(event: KeyboardEvent): string {
	const parts: string[] = [];

	if (event.ctrlKey) parts.push('Ctrl');
	if (event.shiftKey) parts.push('Shift');
	if (event.altKey) parts.push('Alt');
	parts.push(event.key.toUpperCase());

	return parts.join('+');
}

/**
 * Вешает блокировку. Возвращает функцию отписки — вызывать при размонтировании.
 */
export function setupKeybindings(): () => void {
	if (DEV) return () => {};

	const onKeydown = (event: KeyboardEvent) => {
		if (BLOCKED_KEYS.has(event.key) || BLOCKED_COMBINATIONS.has(combination(event))) {
			event.preventDefault();
		}
	};

	const onContextMenu = (event: MouseEvent) => event.preventDefault();

	window.addEventListener('keydown', onKeydown);
	window.addEventListener('contextmenu', onContextMenu);

	return () => {
		window.removeEventListener('keydown', onKeydown);
		window.removeEventListener('contextmenu', onContextMenu);
	};
}
