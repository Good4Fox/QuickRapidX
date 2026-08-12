/**
 * Зовёт колбэк, когда клик пришёлся мимо элемента.
 *
 * Слушатель вешается на следующий кадр: иначе тот самый клик, который открыл
 * панель, всплывёт до document и сразу её закроет.
 */
export function clickOutside(node: HTMLElement, callback: () => void) {
	let current = callback;

	const onClick = (event: MouseEvent) => {
		if (!node.contains(event.target as Node)) current();
	};

	const armed = requestAnimationFrame(() => {
		document.addEventListener('click', onClick);
	});

	return {
		update(next: () => void) {
			current = next;
		},
		destroy() {
			cancelAnimationFrame(armed);
			document.removeEventListener('click', onClick);
		}
	};
}
