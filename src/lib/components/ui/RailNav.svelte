<script lang="ts">
	/*
	 * Навигация с направляющей.
	 *
	 * Тот же приём, что в левом меню главного экрана и в списке разделов
	 * настроек: тонкая линия и бегунок, который едет к выбранному. Здесь она
	 * переключает виды, а не прокручивает к якорям, поэтому отдельным
	 * компонентом, а не ещё одним режимом SettingsNav.
	 */
	type Item = {
		id: string;
		label: string;
		/** Число рядом с подписью: сколько там всего. */
		count?: number;
	};

	type Props = {
		items: Item[];
		value: string;
		onpick: (id: string) => void;
		label: string;
	};

	let { items, value, onpick, label }: Props = $props();

	const index = $derived(Math.max(0, items.findIndex((item) => item.id === value)));
</script>

<nav class="qrx_set_nav" aria-label={label}>
	<div class="qrx_rail" style="--rail-count: {items.length}" aria-hidden="true">
		<div class="qrx_rail_thumb" style="--rail-index: {index}"></div>
	</div>

	{#each items as item (item.id)}
		<button
			class="qrx_set_nav_link"
			class:qrx_set_nav_link_active={item.id === value}
			aria-current={item.id === value ? 'true' : undefined}
			onclick={() => onpick(item.id)}
		>
			<span class="qrx_nav_text">{item.label}</span>

			{#if item.count !== undefined}
				<span class="qrx_nav_count">{item.count}</span>
			{/if}
		</button>
	{/each}
</nav>
