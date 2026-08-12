<script lang="ts">
	/*
	 * Список разделов внутри настроек.
	 *
	 * Своей рамки не имеет — вместо неё направляющая с бегунком, тот же приём,
	 * что и в левом меню главного экрана. Раньше на этом месте стояла колонка,
	 * обведённая золотом в 2px, внутри рамки в 4px: рамка в рамке, о которой и
	 * шла речь. Плюс подсветки текущего места не было вовсе — пункты только
	 * прокручивали к якорю.
	 */

	type Item = {
		/** id заголовка в содержимом, к которому ведёт пункт. */
		id: string;
		label: string;
	};

	type Props = {
		items: Item[];
		/** Прокручиваемая область раздела: за ней следит бегунок. */
		scroller: HTMLElement | null;
	};

	let { items, scroller }: Props = $props();

	let active = $state(0);

	/** Сама колонка — на неё вешается передача колеса. */
	let nav = $state<HTMLElement | null>(null);

	/*
	 * Слежение за прокруткой.
	 *
	 * IntersectionObserver здесь был бы неточен: разделы разной высоты, и при
	 * коротком последнем блоке он не срабатывает до самого низа. Отсчёт по
	 * offsetTop проще и совпадает с тем, что видно на экране.
	 */
	$effect(() => {
		// Область запоминается локально: снятие обработчика должно попасть в ту
		// же, на которую он был повешен, а не в текущую
		const area = scroller;
		if (!area) return;

		const anchors = items
			.map((item) => document.getElementById(item.id))
			.filter((el): el is HTMLElement => el !== null);

		if (anchors.length === 0) return;

		function update() {
			if (!area) return;

			// Докрутили до низа — последний раздел считается текущим, даже если
			// его заголовок остался выше линии отсчёта
			if (area.scrollHeight - area.scrollTop - area.clientHeight < 4) {
				active = anchors.length - 1;
				return;
			}

			const line = area.scrollTop + 24;

			let index = 0;
			anchors.forEach((el, i) => {
				if (el.offsetTop <= line) index = i;
			});

			active = index;
		}

		update();
		area.addEventListener('scroll', update, { passive: true });

		return () => area.removeEventListener('scroll', update);
	});

	/*
	 * Колесо над списком разделов.
	 *
	 * Прокручивается правая колонка, а список — её сосед, а не часть. Курсор над
	 * ним — и колесо крутить нечего: событие уходит вверх, где прокрутки нет.
	 * Поэтому передаём его в ту область, к которой список и относится.
	 *
	 * Обработчик вешается вручную ради `passive: false`: без этого браузер не
	 * даёт отменить событие по умолчанию.
	 */
	$effect(() => {
		const area = scroller;
		const column = nav;
		if (!area || !column) return;

		const onWheel = (event: WheelEvent) => {
			event.preventDefault();
			area.scrollBy({ top: event.deltaY });
		};

		column.addEventListener('wheel', onWheel, { passive: false });
		return () => column.removeEventListener('wheel', onWheel);
	});

	function go(item: Item, index: number) {
		active = index;
		document.getElementById(item.id)?.scrollIntoView({ behavior: 'smooth', block: 'start' });
	}
</script>

<nav class="qrx_set_nav" bind:this={nav}>
	<div class="qrx_rail" style="--rail-count: {items.length}" aria-hidden="true">
		<div class="qrx_rail_thumb" style="--rail-index: {active}"></div>
	</div>

	{#each items as item, index (item.id)}
		<button
			class="qrx_set_nav_link"
			class:qrx_set_nav_link_active={active === index}
			aria-current={active === index ? 'true' : undefined}
			onclick={() => go(item, index)}
		>
			{item.label}
		</button>
	{/each}
</nav>
