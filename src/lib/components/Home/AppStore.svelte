<script lang="ts">
	/*
	 * Витрина winget.
	 *
	 * Открывается подборкой, а не пустым полем поиска: у winget нет понятия
	 * «популярное», он умеет только искать по строке, — поэтому список того,
	 * что видно сразу, наш собственный и выверен вручную.
	 *
	 * Найденное поиском такой пометки не получает. Вместо неё показывается
	 * домен, откуда качается установщик: в winget встречаются пакеты, выложенные
	 * посторонними, и отличает их именно источник загрузки, а не название.
	 *
	 * Разбор таблицы идёт в ядре по положениям колонок: подписи у неё
	 * переведены, а длинные значения слипаются с соседней колонкой.
	 */
	import { invoke } from '@tauri-apps/api/core';
	import { openUrl } from '@tauri-apps/plugin-opener';

	import AppIcon from '$lib/components/ui/AppIcon.svelte';
	import { STORE_GROUPS, STORE_PICKS, type StoreGroup } from '$lib/data/storeCatalog';
	import { basket } from '$lib/state/basket.svelte';
	import { inventory } from '$lib/state/inventory.svelte';
	import { i18n } from '$lib/state/i18n.svelte';
	import { notifyError, notifySuccess } from '$lib/state/notifications.svelte';

	type Package = { id: string; name: string; version: string; source: string };
	type Details = {
		publisher: string;
		publisher_url: string;
		homepage: string;
		installer_url: string;
		license: string;
	};

	let query = $state('');
	let results = $state<Package[] | null>(null);
	let searching = $state(false);
	let installing = $state<string | null>(null);

	let version = $state<string | null>(null);
	let missing = $state(false);

	/** Что уже стоит — по этому списку видно, что ставить не нужно. */
	let installed = $state(new Set<string>());

	/** Раскрытые подробности: ключ — идентификатор пакета. */
	let details = $state<Record<string, Details>>({});
	let opened = $state<string | null>(null);

	/*
	 * Ставить ли в Место.
	 *
	 * winget передаёт папку установщику ключом --location, но послушают его не
	 * все: у одних папка назначения не настраивается вовсе, у других только
	 * своими ключами. Портативные пакеты кладутся туда всегда. Узнать заранее
	 * нельзя, поэтому после установки просто смотрим, появилась ли папка.
	 */
	let toPlace = $state(true);
	let hasPlace = $state(false);

	$effect(() => {
		invoke<{ id: string }[]>('library_list')
			.then((places) => (hasPlace = places.length > 0))
			.catch(() => undefined);
	});

	$effect(() => {
		basket.restore();
		inventory.ensure();
	});

	/*
	 * Значков у winget нет вовсе. Но то, что уже стоит на машине, осмотр нашёл
	 * вместе с путём к исполняемому файлу — оттуда значок и берётся. Для
	 * остального остаётся буква названия.
	 */
	function iconPath(name: string): string {
		const needle = name.toLowerCase();

		const match = inventory.findings.find(
			(finding) => finding.exe && finding.name.toLowerCase().includes(needle)
		);

		return match?.exe ?? '';
	}

	$effect(() => {
		invoke<string>('winget_version')
			.then((value) => (version = value))
			.catch(() => (missing = true));

		invoke<Package[]>('winget_installed')
			.then((list) => (installed = new Set(list.map((item) => item.id.toLowerCase()))))
			.catch(() => undefined);
	});

	const groupLabel: Record<StoreGroup, () => string> = {
		browsers: () => i18n.t.store_g_browsers,
		dev: () => i18n.t.store_g_dev,
		media: () => i18n.t.store_g_media,
		chat: () => i18n.t.store_g_chat,
		tools: () => i18n.t.store_g_tools,
		archive: () => i18n.t.store_g_archive
	};

	async function search() {
		const needle = query.trim();

		if (!needle) {
			results = null;
			return;
		}

		if (searching) return;
		searching = true;

		try {
			results = await invoke<Package[]>('winget_search', { query: needle });
		} catch (e) {
			notifyError(i18n.t.notification_text_4, String(e));
			results = [];
		}

		searching = false;
	}

	async function install(id: string, name: string) {
		if (installing) return;
		installing = id;

		let location = '';

		if (toPlace && hasPlace) {
			location = await invoke<string>('library_install_path', { name }).catch(() => '');
		}

		try {
			await invoke('winget_install', { id, location: location || null });
			installed = new Set([...installed, id.toLowerCase()]);

			// Установщик мог не послушать ключ — проверяем, а не верим на слово
			const landed = location ? await invoke<boolean>('library_has', { path: location }) : false;

			notifySuccess(
				i18n.t.store_installed,
				location ? (landed ? i18n.t.store_placed : i18n.t.store_not_placed) : name
			);
		} catch (e) {
			notifyError(name, String(e));
		}

		installing = null;
	}

	/** Подробности спрашиваются по требованию: это отдельный вызов winget. */
	async function toggleDetails(id: string) {
		opened = opened === id ? null : id;

		if (opened !== id || details[id]) return;

		try {
			details = { ...details, [id]: await invoke<Details>('winget_show', { id }) };
		} catch (e) {
			console.error('сведения не получены:', e);
		}
	}

	/** Домен из ссылки — по нему и видно, откуда на самом деле качается. */
	function domain(url: string): string {
		try {
			return new URL(url).hostname.replace(/^www\./, '');
		} catch {
			return '';
		}
	}

	function onKey(event: KeyboardEvent) {
		if (event.key === 'Enter') search();
	}
</script>

<div class="qrx_found">
	<header class="qrx_found_head">
		<div class="qrx_set_row_text">
			<span class="qrx_set_row_title">{i18n.t.store_title}</span>
			<span class="qrx_set_row_note">{missing ? i18n.t.store_missing : i18n.t.store_note}</span>
		</div>

		{#if version}
			<span class="qrx_found_version">winget {version}</span>
		{/if}
	</header>

	{#if !missing}
		<div class="qrx_found_tools">
			<input
				class="qrx_found_search"
				type="text"
				placeholder={i18n.t.store_search}
				bind:value={query}
				onkeydown={onKey}
				spellcheck="false"
			/>

			<button class="qrx_sys_link" disabled={searching} onclick={search}>
				{searching ? i18n.t.store_searching : i18n.t.store_find}
			</button>
		</div>

		<!-- Куда ставить: в наше Место или обычным путём в систему -->
		<label class="qrx_place_switch">
			<input class="qrx_toggle" type="checkbox" bind:checked={toPlace} disabled={!hasPlace} />

			<span class="qrx_set_row_text">
				<span class="qrx_set_row_title">{i18n.t.store_to_place}</span>
				<span class="qrx_rec_task_note">
					{hasPlace ? i18n.t.store_to_place_note : i18n.t.store_place_none}
				</span>
			</span>
		</label>

		{#snippet card(id: string, name: string, version: string, checked: boolean)}
			<article class="qrx_pkg" class:qrx_pkg_open={opened === id}>
				<div class="qrx_pkg_top">
					<AppIcon path={iconPath(name)} fallback={name} size={56} />

					<div class="qrx_found_main">
						<span class="qrx_found_name">{name}</span>
						<span class="qrx_rec_task_note">{id}</span>
					</div>

					{#if checked}
						<span
							class="qrx_found_tag qrx_found_tag_portable"
							data-tooltip={i18n.t.store_checked_note}
							data-tooltip-position="top"
						>
							{i18n.t.store_checked}
						</span>
					{/if}

					{#if version}
						<span class="qrx_found_version">{version}</span>
					{/if}

					<button class="qrx_rec_log" onclick={() => toggleDetails(id)}>
						{i18n.t.store_details}
					</button>

					<!-- Плюсик кладёт в корзину: скачать пакетно, ничего не ставя -->
					<button
						class="qrx_plus"
						class:qrx_plus_on={basket.has(id)}
						aria-label={basket.has(id) ? i18n.t.bag_added : i18n.t.bag_add}
						data-tooltip={basket.has(id) ? i18n.t.bag_added : i18n.t.bag_add}
						data-tooltip-position="top"
						onclick={() => basket.toggle({ key: id, name, packageId: id, url: '' })}
					>
						{basket.has(id) ? '✓' : '+'}
					</button>

					{#if installed.has(id.toLowerCase())}
						<span class="qrx_found_tag qrx_found_tag_installed">{i18n.t.store_have}</span>
					{:else}
						<button
							class="qrx_sys_link"
							disabled={installing !== null}
							onclick={() => install(id, name)}
						>
							{installing === id ? i18n.t.store_installing : i18n.t.store_install}
						</button>
					{/if}
				</div>

				{#if opened === id}
					<div class="qrx_pkg_details">
						{#if details[id]}
							{#if details[id].publisher}
								<span class="qrx_rec_task_note">
									{i18n.t.store_publisher}: {details[id].publisher}
								</span>
							{/if}

							{#if details[id].installer_url}
								<!-- Домен установщика — то, что отличает настоящий пакет от копии -->
								<button
									class="qrx_pkg_source"
									onclick={() => openUrl(details[id].installer_url)}
								>
									{i18n.t.store_from} {domain(details[id].installer_url)}
								</button>
							{/if}

							{#if details[id].homepage}
								<button class="qrx_rec_log" onclick={() => openUrl(details[id].homepage)}>
									{domain(details[id].homepage)}
								</button>
							{/if}
						{:else}
							<span class="qrx_rec_task_note">…</span>
						{/if}
					</div>
				{/if}
			</article>
		{/snippet}

		{#if results !== null}
			<span class="qrx_rec_task_note">{i18n.t.store_results}: {results.length}</span>
			<span class="qrx_rec_warn">{i18n.t.store_warn}</span>

			<div class="qrx_pkg_list">
				{#each results as pkg (pkg.id)}
					{@render card(pkg.id, pkg.name, pkg.version, false)}
				{/each}
			</div>
		{:else}
			{#each STORE_GROUPS as group (group)}
				<span class="qrx_pkg_group">{groupLabel[group]()}</span>

				<div class="qrx_pkg_list">
					{#each STORE_PICKS.filter((pick) => pick.group === group) as pick (pick.id)}
						{@render card(pick.id, pick.name, '', true)}
					{/each}
				</div>
			{/each}
		{/if}
	{/if}
</div>
