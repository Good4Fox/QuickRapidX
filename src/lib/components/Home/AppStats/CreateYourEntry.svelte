<script lang="ts">
	/*
	 * Создание своей записи. Форма — та же, что в редакторе: в исходном
	 * приложении её разметка была продублирована один в один.
	 */
	import AppStatsInfoEditor from '$lib/components/Home/AppStats/AppStatsInfoEditor.svelte';
	import { blankEntry, createEntry } from '$lib/state/appStore';
	import type { AppEntry } from '$lib/data/appTypes';

	type Props = {
		/** Категория, в которую ляжет запись. */
		category: string;
		oncancel: () => void;
		oncreated: () => void;
	};

	let { category, oncancel, oncreated }: Props = $props();

	let entry = $state<AppEntry>(blankEntry());

	async function save(value: AppEntry) {
		if (await createEntry(category, value)) oncreated();
	}
</script>

<AppStatsInfoEditor bind:entry {oncancel} onsave={save} />
