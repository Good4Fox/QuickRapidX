<script lang="ts">
	/*
	 * Домашний экран: узкое левое меню разделов и область содержимого справа.
	 * Разметка и классы перенесены из исходного приложения.
	 */
	import HomeApp from '$lib/components/Home/HomeApp.svelte';
	import HomeAutomation from '$lib/components/Home/HomeAutomation.svelte';
	import HomeStart from '$lib/components/Home/HomeStart.svelte';
	import ViewSwitch from '$lib/components/ViewSwitch.svelte';
	import { i18n } from '$lib/state/i18n.svelte';

	type View = 'home' | 'app' | 'automation';

	let view = $state<View>('home');

	/**
	 * Куда идёт переход. Разделы выстроены в меню сверху вниз, поэтому
	 * движение вправо означает «ниже по списку», влево — «выше».
	 */
	const ORDER: View[] = ['home', 'app', 'automation'];

	let direction = $state(1);

	function go(next: View) {
		direction = ORDER.indexOf(next) >= ORDER.indexOf(view) ? 1 : -1;
		view = next;
	}

	/** Номер выбранного раздела — по нему бегунок встаёт напротив плитки. */
	const activeIndex = $derived(ORDER.indexOf(view));

	/** Готовые разделы: номер иконки, подпись и внутреннее имя. */
	const ready: { id: View; ico: number; tip: () => string }[] = [
		{ id: 'home', ico: 1, tip: () => i18n.t.home_text_1 },
		{ id: 'app', ico: 2, tip: () => i18n.t.home_text_2 },
		{ id: 'automation', ico: 3, tip: () => i18n.t.home_text_3 }
	];

	/**
	 * Разделы, которых ещё нет. В исходном приложении они выглядели так же,
	 * как рабочие, — здесь приглушены и не отвечают на наведение.
	 */
	const pending = [
		{ ico: 4, tip: () => i18n.t.home_text_4 },
		{ ico: 5, tip: () => i18n.t.home_text_5 },
		{ ico: 6, tip: () => i18n.t.home_text_6 },
		{ ico: 7, tip: () => i18n.t.home_text_7 },
		{ ico: 8, tip: () => i18n.t.home_text_8 },
		{ ico: 9, tip: () => i18n.t.home_text_9 }
	];
</script>

<div class="user_home">
	<div class="user_home_left">
		<div class="user_home_left_use user_home_left_group">
			<!-- Направляющая с бегунком: едет к выбранному разделу -->
			<div class="qrx_rail" style="--rail-count: {ready.length}" aria-hidden="true">
				<div class="qrx_rail_thumb" style="--rail-index: {activeIndex}"></div>
			</div>

			{#each ready as item (item.id)}
				<button
					class="user_home_left_link"
					class:user_home_left_link_active={view === item.id}
					aria-label={item.tip()}
					aria-current={view === item.id ? 'page' : undefined}
					onclick={() => go(item.id)}
				>
					<div
						class="user_home_left_link_back"
						data-tooltip={item.tip()}
						data-tooltip-position="right"
					>
						<div class="user_home_left_link_back_ico_{item.ico}"></div>
					</div>
				</button>
			{/each}
		</div>

		<div class="user_home_left_use">
			{#each pending as item (item.ico)}
				<div class="user_home_left_link user_home_left_link_soon">
					<div
						class="user_home_left_link_back"
						data-tooltip={item.tip()}
						data-tooltip-position="right"
					>
						<div class="user_home_left_link_back_ico_{item.ico}"></div>
					</div>
				</div>
			{/each}
		</div>
	</div>

	<div class="user_home_right">
		<div class="user_home_right_panel">
			<ViewSwitch {view} {direction}>
				{#if view === 'home'}
					<HomeStart ongo={go} />
				{:else if view === 'app'}
					<HomeApp />
				{:else if view === 'automation'}
					<HomeAutomation />
				{/if}
			</ViewSwitch>
		</div>
	</div>
</div>
