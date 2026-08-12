<script lang="ts">
	/*
	 * «Капля» по центру полосы заголовка.
	 *
	 * Раньше здесь была просто полоска-признак того, что за это место можно
	 * тащить окно. Теперь это общая витрина состояния: в покое — та же полоска,
	 * а когда есть что показать, она раскрывается в плашку с содержимым и
	 * сама сворачивается обратно.
	 *
	 * Сюда же будут добавляться датчики; поэтому вид определяется не флагами,
	 * а очередью: кто-то кладёт в неё сообщение, капля показывает и убирает.
	 */
	import { fly } from 'svelte/transition';

	import LevelIcon from '$lib/components/Notifications/LevelIcon.svelte';
	import { boot, bootLabel } from '$lib/state/boot.svelte';
	import { i18n } from '$lib/state/i18n.svelte';
	import { island } from '$lib/state/island.svelte';
	import { notifications } from '$lib/state/notifications.svelte';

	/** Последнее уведомление, которое капля уже показала. */
	let shownId = $state<string | null>(null);

	// Новое уведомление — показываем его в капле. Так о событии видно, даже
	// если всплывающее окошко закрыто или ушло.
	$effect(() => {
		const latest = notifications.items[0];
		if (!latest || latest.id === shownId) return;

		shownId = latest.id;
		island.show({
			kind: latest.level,
			title: latest.title,
			text: latest.text
		});
	});

	// Пока идёт загрузка ядра, капля показывает её шаг.
	$effect(() => {
		if (boot.ready || boot.total === 0) return;
		island.pin({ kind: 'progress', title: bootLabel(i18n.t, boot.key), percent: boot.percent });
	});

	$effect(() => {
		if (boot.ready) island.unpin();
	});
</script>

<div
	class="qrx_island"
	class:qrx_island_open={island.current !== null}
	data-tauri-drag-region
	role="status"
	aria-live="polite"
>
	{#if island.current}
		{@const item = island.current}
		<div class="qrx_island_body" transition:fly={{ y: -6, duration: 180 }}>
			<span class="qrx_island_icon qrx_note_{item.kind === 'progress' ? 'info' : item.kind}">
				{#if item.kind === 'progress'}
					<span class="qrx_island_spinner"></span>
				{:else}
					<LevelIcon level={item.kind} size={14} />
				{/if}
			</span>

			<span class="qrx_island_title">{item.title}</span>

			{#if item.kind === 'progress' && item.percent !== undefined}
				<span class="qrx_island_percent">{item.percent}%</span>
			{/if}
		</div>
	{:else}
		<div class="qrx_island_idle" data-tauri-drag-region></div>
	{/if}
</div>
