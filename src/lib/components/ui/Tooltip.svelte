<script lang="ts">
	/*
	 * Подсказка при наведении — одна на окно.
	 *
	 * Было: псевдоэлемент `::after` внутри самого элемента. Два изъяна, и оба
	 * неисправимы на своём месте. Он рос в одну строку какой угодно ширины —
	 * длинная подсказка у небольшой карточки уезжала за оба края сразу. И он
	 * жил внутри прокручиваемой области, а та обрезает всё, что вышло за её
	 * границы: подсказка у верхнего ряда срезалась по краю списка.
	 *
	 * Стало: одна коробка, лежащая вне всякой прокрутки, с положением по
	 * месту. Она сама переворачивается на другую сторону, когда с выбранной не
	 * помещается, и в последнюю очередь подпирается к краю окна — за пределы
	 * не уходит никогда.
	 *
	 * Разметка нигде не менялась: подсказки как были на `data-tooltip`, так и
	 * остались, их около сорока по всем окнам.
	 */
	import { tick } from 'svelte';

	/** Отступ от элемента, к которому подсказка относится. */
	const GAP = 6;
	/** Поле у краёв окна: вплотную к ним подсказка не подходит. */
	const EDGE = 8;
	/**
	 * Задержка появления. Проводя курсор над рядом кнопок, подсказки видеть не
	 * нужно — без задержки они дёргались бы одна за другой.
	 */
	const DELAY = 400;

	type Side = 'top' | 'bottom' | 'left' | 'right';

	const FLIP: Record<Side, Side> = {
		top: 'bottom',
		bottom: 'top',
		left: 'right',
		right: 'left'
	};

	let box = $state<HTMLDivElement | null>(null);
	let text = $state('');
	let shown = $state(false);
	let left = $state(0);
	let top = $state(0);

	/** Элемент, к которому подсказка сейчас относится. */
	let anchor: HTMLElement | null = null;
	let timer: ReturnType<typeof setTimeout> | null = null;

	function hide() {
		if (timer) {
			clearTimeout(timer);
			timer = null;
		}

		anchor = null;

		/*
		 * Снятие откладывается на микрозадачу, и это не украшательство.
		 *
		 * `focusout` приходит и в тот миг, когда разметку под фокусом разбирают:
		 * ушёл со страницы элемент — ушёл и фокус. Обработчик при этом
		 * вызывается прямо посреди разборки, а менять состояние внутри неё
		 * Svelte запрещает — `state_unsafe_mutation`. Ровно это и вылезло в
		 * работающем приложении при переходе между списком наборов и правкой.
		 *
		 * Отметка о том, к чему подсказка относится, снимается сразу: она
		 * обычная переменная, и по ней же отличается «спрятать насовсем» от
		 * «спрятать и тут же показать другую» — во втором случае снимать уже
		 * нечего.
		 */
		queueMicrotask(() => {
			if (!anchor) shown = false;
		});
	}

	/** Помещается ли подсказка с этой стороны, не залезая на край окна. */
	function fits(side: Side, near: DOMRect, size: DOMRect): boolean {
		switch (side) {
			case 'top':
				return near.top - GAP - size.height >= EDGE;
			case 'bottom':
				return near.bottom + GAP + size.height <= window.innerHeight - EDGE;
			case 'left':
				return near.left - GAP - size.width >= EDGE;
			case 'right':
				return near.right + GAP + size.width <= window.innerWidth - EDGE;
		}
	}

	function clamp(value: number, low: number, high: number): number {
		return Math.max(low, Math.min(value, high));
	}

	async function place(target: HTMLElement) {
		const label = target.getAttribute('data-tooltip') ?? '';

		if (!label) return;

		text = label;

		// Размер известен только после отрисовки: пока текст не встал на место,
		// мерить нечего, а от размера зависит и сторона, и положение
		await tick();

		// За время отрисовки курсор мог уйти — тогда ставить уже нечего
		if (anchor !== target || !box) return;

		const near = target.getBoundingClientRect();
		const size = box.getBoundingClientRect();

		const asked = target.getAttribute('data-tooltip-position') as Side | null;
		const wanted: Side = asked && asked in FLIP ? asked : 'top';

		// Сторону меняем только если с обратной действительно лучше: иначе
		// подсказка прыгала бы туда-сюда без причины
		const side = fits(wanted, near, size)
			? wanted
			: fits(FLIP[wanted], near, size)
				? FLIP[wanted]
				: wanted;

		if (side === 'top' || side === 'bottom') {
			left = near.left + near.width / 2 - size.width / 2;
			top = side === 'top' ? near.top - GAP - size.height : near.bottom + GAP;
		} else {
			top = near.top + near.height / 2 - size.height / 2;
			left = side === 'left' ? near.left - GAP - size.width : near.right + GAP;
		}

		// Последний рубеж. Ни поворот, ни выравнивание по центру не спасают
		// элемент у самого края — а видной подсказка должна быть целиком
		left = clamp(left, EDGE, window.innerWidth - EDGE - size.width);
		top = clamp(top, EDGE, window.innerHeight - EDGE - size.height);

		shown = true;
	}

	function enter(target: EventTarget | null) {
		const found =
			target instanceof Element
				? (target.closest('[data-tooltip]') as HTMLElement | null)
				: null;

		if (found === anchor) return;

		hide();

		// Подсказки без текста быть не должно: пустая коробка сбивает с толку
		// сильнее, чем её отсутствие
		if (!found?.getAttribute('data-tooltip')) return;

		anchor = found;
		timer = setTimeout(() => place(found), DELAY);
	}

	$effect(() => {
		const over = (event: PointerEvent) => enter(event.target);
		const focused = (event: FocusEvent) => enter(event.target);

		/*
		 * Слушаем на перехвате и на самом документе: подсказки раскиданы по
		 * всем окнам, и ни одно из них не должно об этом знать. Прокрутка —
		 * тоже на перехвате: она случается внутри списков, а те всплытия не
		 * дают.
		 */
		document.addEventListener('pointerover', over, true);
		document.addEventListener('focusin', focused, true);
		document.addEventListener('pointerdown', hide, true);
		document.addEventListener('focusout', hide, true);
		document.addEventListener('scroll', hide, true);
		window.addEventListener('blur', hide);
		window.addEventListener('resize', hide);

		return () => {
			document.removeEventListener('pointerover', over, true);
			document.removeEventListener('focusin', focused, true);
			document.removeEventListener('pointerdown', hide, true);
			document.removeEventListener('focusout', hide, true);
			document.removeEventListener('scroll', hide, true);
			window.removeEventListener('blur', hide);
			window.removeEventListener('resize', hide);

			if (timer) clearTimeout(timer);
		};
	});
</script>

<div
	bind:this={box}
	class="qrx_tip"
	class:qrx_tip_shown={shown}
	style="left: {left}px; top: {top}px"
	aria-hidden="true"
>
	{text}
</div>
