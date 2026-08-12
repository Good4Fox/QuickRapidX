<script lang="ts">
	/*
	 * Выпадающий список.
	 *
	 * Системный <select> оформлению не поддаётся: список рисует Windows, и он
	 * приходит со своим фоном, своим шрифтом и прямыми углами — посреди тёмного
	 * окна с золотом это выглядело чужой деталью. Здесь список свой.
	 *
	 * Раскладка ролей та же, что у настоящего элемента: кнопка с
	 * aria-haspopup="listbox" и список с role="listbox", поэтому с клавиатуры и
	 * для программ чтения с экрана он остаётся списком, а не набором кнопок.
	 */

	type Option = {
		value: string;
		label: string;
		/** Вторая строка пункта: чем этот вариант отличается. */
		note?: string;
	};

	type Props = {
		options: Option[];
		value: string;
		onpick: (value: string) => void;
		/** Подпись для программ чтения с экрана. */
		label: string;
		/** Открывать список вверх — если снизу места нет. */
		up?: boolean;
	};

	let { options, value, onpick, label, up = false }: Props = $props();

	let open = $state(false);
	let root = $state<HTMLElement | null>(null);
	let list = $state<HTMLElement | null>(null);

	/** Пункт под курсором клавиатуры — не то же самое, что выбранный. */
	let cursor = $state(0);

	const id = $props.id();

	const current = $derived(options.find((option) => option.value === value) ?? options[0]);
	const selectedIndex = $derived(Math.max(0, options.findIndex((o) => o.value === value)));

	// Щелчок мимо закрывает список. Слушаем на фазе перехвата: иначе кнопка
	// внутри успела бы обработать щелчок и снова открыть его.
	$effect(() => {
		if (!open) return;

		const onDown = (event: PointerEvent) => {
			if (root && !root.contains(event.target as Node)) open = false;
		};

		document.addEventListener('pointerdown', onDown, true);
		return () => document.removeEventListener('pointerdown', onDown, true);
	});

	// Открыли — ставим курсор на выбранный пункт и уводим на него фокус
	$effect(() => {
		if (!open) return;
		cursor = selectedIndex;
		queueMicrotask(() => list?.focus());
	});

	function toggle() {
		open = !open;
	}

	function pick(index: number) {
		const option = options[index];
		if (!option) return;

		open = false;
		if (option.value !== value) onpick(option.value);
	}

	function onKey(event: KeyboardEvent) {
		switch (event.key) {
			case 'Escape':
				event.preventDefault();
				open = false;
				break;
			case 'ArrowDown':
				event.preventDefault();
				cursor = (cursor + 1) % options.length;
				break;
			case 'ArrowUp':
				event.preventDefault();
				cursor = (cursor - 1 + options.length) % options.length;
				break;
			case 'Home':
				event.preventDefault();
				cursor = 0;
				break;
			case 'End':
				event.preventDefault();
				cursor = options.length - 1;
				break;
			case 'Enter':
			case ' ':
				event.preventDefault();
				pick(cursor);
				break;
		}
	}

	function onButtonKey(event: KeyboardEvent) {
		if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;
		event.preventDefault();
		open = true;
	}
</script>

<div class="qrx_select" class:qrx_select_open={open} bind:this={root}>
	<button
		class="qrx_select_head"
		aria-haspopup="listbox"
		aria-expanded={open}
		aria-label={label}
		onclick={toggle}
		onkeydown={onButtonKey}
	>
		<span class="qrx_select_value">{current?.label ?? ''}</span>
		<span class="qrx_select_arrow" aria-hidden="true"></span>
	</button>

	{#if open}
		<!--
			tabindex на самом списке, а не на пунктах: перебор стрелками ведёт
			курсор внутри одного элемента, как в системном списке, и Tab не
			проходит по каждому варианту отдельно.
		-->
		<div
			class="qrx_select_list"
			class:qrx_select_list_up={up}
			role="listbox"
			aria-label={label}
			aria-activedescendant="{id}-{cursor}"
			tabindex="-1"
			bind:this={list}
			onkeydown={onKey}
		>
			{#each options as option, index (option.value)}
				<div
					class="qrx_select_option"
					class:qrx_select_option_cursor={index === cursor}
					class:qrx_select_option_active={option.value === value}
					id="{id}-{index}"
					role="option"
					aria-selected={option.value === value}
					tabindex="-1"
					onpointerenter={() => (cursor = index)}
					onclick={() => pick(index)}
					onkeydown={onKey}
				>
					<span class="qrx_select_option_text">
						<span class="qrx_select_option_label">{option.label}</span>
						{#if option.note}
							<span class="qrx_select_option_note">{option.note}</span>
						{/if}
					</span>

					{#if option.value === value}
						<span class="qrx_select_tick" aria-hidden="true"></span>
					{/if}
				</div>
			{/each}
		</div>
	{/if}
</div>
