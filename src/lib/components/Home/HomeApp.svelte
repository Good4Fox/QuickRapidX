<script lang="ts">
	/*
	 * Раздел «Приложения»: поиск сверху, содержимое по центру и выдвижная
	 * панель категорий снизу. Разметка и классы — из исходного приложения.
	 */
	import { invoke } from '@tauri-apps/api/core';
	import { listen } from '@tauri-apps/api/event';
	import { open } from '@tauri-apps/plugin-dialog';

	import AppFound from '$lib/components/Home/AppFound.svelte';
	import AppGroups from '$lib/components/Home/AppGroups.svelte';
	import AppBasket from '$lib/components/Home/AppBasket.svelte';
	import AppStore from '$lib/components/Home/AppStore.svelte';
	import AppLibrary from '$lib/components/Home/AppLibrary.svelte';
	import LibraryOffer from '$lib/components/Home/LibraryOffer.svelte';
	import CatalogEditor from '$lib/components/Home/CatalogEditor.svelte';
	import CatalogList from '$lib/components/Home/CatalogList.svelte';
	import RailNav from '$lib/components/ui/RailNav.svelte';
	import ViewSwitch from '$lib/components/ViewSwitch.svelte';
	import { notifyError, notifySuccess } from '$lib/state/notifications.svelte';
	import { basket } from '$lib/state/basket.svelte';
	import { groups } from '$lib/state/groups.svelte';
	import { inventory } from '$lib/state/inventory.svelte';
	import { blankEntry } from '$lib/state/appStore';
	import type { AppEntry } from '$lib/data/appTypes';
	import { i18n } from '$lib/state/i18n.svelte';
	import { loadUses } from '$lib/state/welcome.svelte';

	/** Категории нижней панели: ключ раздела, номер иконки и подпись. */
	const CATEGORIES = [
		{ key: 'app_dops', ico: 1, stats: 'App', tip: () => i18n.t.AppTextUse1_1 },
		{ key: 'app_games_dops', ico: 2, stats: 'Games', tip: () => i18n.t.AppTextUse1_2 },
		{ key: 'app_game_launcher_dops', ico: 3, stats: 'Games Launcher', tip: () => i18n.t.AppTextUse1_3 },
		{ key: 'app_dev_dops', ico: 4, stats: 'Dev', tip: () => i18n.t.AppTextUse1_4 },
		{ key: 'app_ai_dops', ico: 5, stats: 'AI', tip: () => i18n.t.AppTextUse1_5 }
		] as const;

	type CategoryKey = (typeof CATEGORIES)[number]['key'];

	/*
	 * С какого раздела открыться.
	 *
	 * Берётся из того, что человек отметил в приветственном мастере. Прежде те
	 * галочки не читались нигде: мастер спрашивал, чем человек пользуется, и
	 * забывал ответ на следующем же шаге. Отвечает пока только «Игры» — прочие
	 * пять способов ложатся на общий раздел, потому что делить его нам ещё не
	 * на что.
	 */
	function firstView(): CategoryKey {
		return loadUses().includes('games') ? 'app_games_dops' : 'app_dops';
	}

	let active = $state<CategoryKey>(firstView());

	/*
	 * Хранилище — не категория программ, а место, куда они кладутся, поэтому
	 * оно отдельным видом, а не седьмой кнопкой в ряду категорий.
	 */
	let view = $state<'catalog' | 'store' | 'basket' | 'found' | 'groups' | 'library'>('catalog');

	/*
	 * Из панели трея просят открыть наборы: там их ещё нет, и заводить их надо
	 * здесь. Без этого кнопка приводила бы в общий каталог, где наборов не
	 * видно, — и звала бы в никуда.
	 */
	$effect(() => {
		const asked = listen('app:open-groups', () => (view = 'groups'));

		return () => {
			asked.then((stop) => stop());
		};
	});

	/*
	 * Переход между разделами едет вбок — как на главном экране.
	 *
	 * Направление берётся из порядка в левом ряду: вниз по списку значит
	 * вправо. Так движение совпадает с тем, что человек только что сделал
	 * мышью, и переход читается как перелистывание, а не как подмена.
	 */
	const ORDER = ['catalog', 'store', 'basket', 'found', 'groups', 'library'] as const;

	let direction = $state(1);

	function pick(next: typeof view) {
		/*
		 * Любой переход по левому ряду закрывает открытый набор.
		 *
		 * Раздел должен встречать списком, а не тем, что осталось с прошлого
		 * захода: уйдя в «Магазин» и вернувшись, человек ждёт наборы, а не
		 * правку одного из них. Заодно это и «назад» повторным нажатием на
		 * «Наборы» — тянуться за кнопкой в заголовке не нужно.
		 *
		 * Терять при этом нечего: в наборе всё сохраняется сразу. Форму записи
		 * каталога так закрывать нельзя — там лежит невведённый текст, поэтому
		 * её не трогаем.
		 */
		groups.open = '';

		direction = ORDER.indexOf(next) >= ORDER.indexOf(view) ? 1 : -1;
		view = next;
	}

	/** Поиск по каталогу. Поле раньше было подписано «Скоро» и не работало. */
	let query = $state('');

	/**
	 * Перенос каталога из прежней версии.
	 *
	 * У старого приложения он лежал рядом с исполняемым файлом, а не в каталоге
	 * данных, поэтому при переезде остался там. Схема совпадает — достаточно
	 * указать файл.
	 */
	async function saveEntry(entry: AppEntry) {
		try {
			await invoke('catalog_save', { category: active, entry });
			notifySuccess(i18n.t.cat_saved, entry.programm_name);
			editing = null;
			await load();
		} catch (e) {
			notifyError(i18n.t.notification_text_4, String(e));
		}
	}

	async function removeEntry() {
		if (!editing) return;

		try {
			await invoke('catalog_remove', { category: active, id: editing.id });
			notifySuccess(i18n.t.cat_deleted, editing.programm_name);
			editing = null;
			await load();
		} catch (e) {
			notifyError(i18n.t.notification_text_4, String(e));
		}
	}

	async function importCatalog() {
		const picked = await open({
			multiple: false,
			title: i18n.t.cat_import,
			filters: [{ name: 'app_data.json', extensions: ['json'] }]
		});

		if (typeof picked !== 'string') return;

		try {
			const added = await invoke<number>('import_catalog', { path: picked });
			notifySuccess(i18n.t.cat_imported, String(added));
			await load();
		} catch (e) {
			notifyError(i18n.t.notification_text_4, String(e));
		}
	}

	/**
	 * Предложение завести Место при первом заходе.
	 *
	 * Показывается, пока мест нет и человек не отказался. Отказ помнится, чтобы
	 * предложение не повторялось при каждом открытии раздела.
	 */
	const OFFER_KEY = 'AppLibraryOfferSeen';

	let offer = $state(false);

	$effect(() => {
		if (localStorage.getItem(OFFER_KEY) === 'yes') return;

		invoke<unknown[]>('library_list')
			.then((places) => (offer = places.length === 0))
			.catch(() => undefined);
	});

	function closeOffer() {
		localStorage.setItem(OFFER_KEY, 'yes');
		offer = false;
	}
	/** Открытая форма: пусто — показывается список. */
	let editing = $state<AppEntry | null>(null);
	let movedDown = $state(false);
	let search = $state('');

	/** Сохранённые записи по категориям. Приходят из ядра. */
	let entries = $state<Record<string, AppEntry[]>>({});

	const current = $derived(CATEGORIES.find((c) => c.key === active)!);

	/** Записи выбранной категории после поиска. */
	const shown = $derived.by(() => {
		const list = entries[active] ?? [];
		const needle = query.trim().toLowerCase();

		if (!needle) return list;

		return list.filter(
			(entry) =>
				entry.programm_name.toLowerCase().includes(needle) ||
				entry.programm_description.toLowerCase().includes(needle)
		);
	});

	/*
	 * Числа рядом с разделами.
	 *
	 * Наборы читаются из одного файла — это мгновенно, поэтому список
	 * поднимается сразу при входе в раздел, и число верно с первого взгляда.
	 *
	 * Осмотр машины идёт секунды: реестр, пакеты Store, пути, портативные
	 * папки. Пока он не кончился, числа нет вовсе — вместо него пусто.
	 * Показывать ноль было бы прямой неправдой: не «ничего не нашли», а «ещё
	 * не искали».
	 */
	const SECTIONS = $derived([
		{ id: 'catalog', label: i18n.t.app_catalog, count: entries[active]?.length ?? 0 },
		{ id: 'store', label: i18n.t.app_store },
		{ id: 'basket', label: i18n.t.bag_title, count: basket.items.length },
		{
			id: 'found',
			label: i18n.t.app_found,
			count: inventory.findings.length > 0 ? inventory.findings.length : undefined
		},
		{ id: 'groups', label: i18n.t.grp_title, count: groups.list.length },
		{ id: 'library', label: i18n.t.lib_title }
	]);

	$effect(() => {
		load();

		// Наборы и корзина читаются из файлов и поднимаются сразу: их числа
		// должны быть верны с первого взгляда, а не после захода в раздел
		groups.ensure();
		basket.restore();
	});

	async function load() {
		try {
			const raw = await invoke<string>('json_data_info');
			const parsed: AppEntry[][] = JSON.parse(raw);

			// Ядро отдаёт пять списков в порядке категорий
			entries = {
				app_dops: parsed[0] ?? [],
				app_games_dops: parsed[1] ?? [],
				app_game_launcher_dops: parsed[2] ?? [],
				app_dev_dops: parsed[3] ?? [],
				app_ai_dops: parsed[4] ?? []
			};
		} catch (e) {
			// Слой хранения переносится следующим этапом — до тех пор списки пусты
			console.warn('json_data_info недоступна:', e);
		}
	}
</script>

<div class="qrx_app">
	{#if offer}
		<LibraryOffer ondone={closeOffer} />
	{:else}
		<!-- Повторное нажатие на раздел работает как «назад»: в наборах это
		     выход к списку, и тянуться за кнопкой в заголовке не нужно -->
		<RailNav
			items={SECTIONS}
			value={view}
			onpick={(id) => pick(id as typeof view)}
			label={i18n.t.home_text_2}
		/>

		<div class="qrx_app_body">
			<ViewSwitch {view} {direction}>
			{#if view === 'library'}
				<AppLibrary />
			{:else if view === 'store'}
				<AppStore />
			{:else if view === 'basket'}
				<AppBasket />
			{:else if view === 'found'}
				<AppFound />
			{:else if view === 'groups'}
				<AppGroups />
			{:else}
				<div class="qrx_cat">
					<header class="qrx_cat_head">
						<input
							class="qrx_found_search"
							type="text"
							placeholder={i18n.t.appTextUse2}
							bind:value={query}
							spellcheck="false"
						/>

						<button class="qrx_sys_link" onclick={() => (editing = blankEntry())}>
							{i18n.t.cat_new}
						</button>

						<button
							class="qrx_rec_log"
							data-tooltip={i18n.t.cat_import_note}
							data-tooltip-position="bottom"
							onclick={importCatalog}
						>
							{i18n.t.cat_import}
						</button>
					</header>

					{#if editing}
						<CatalogEditor
							entry={editing}
							editing={(entries[active] ?? []).some((each) => each.id === editing?.id)}
							onsave={saveEntry}
							ondelete={removeEntry}
							oncancel={() => (editing = null)}
						/>
					{:else}
						<!-- Категории были спрятаны в выдвижной панели снизу -->
						<div class="qrx_found_kinds">
							{#each CATEGORIES as category (category.key)}
								<button
									class="qrx_found_chip"
									class:qrx_found_chip_active={active === category.key}
									onclick={() => (active = category.key)}
								>
									{category.tip()}
									<span class="qrx_nav_count">{entries[category.key]?.length ?? 0}</span>
								</button>
							{/each}
						</div>

						<span class="qrx_rec_task_note">
							<!-- Подпись раздела берётся тем же ключом, что и подсказка чипа
						     рядом: поле `stats` служебное и по-английски -->
						{shown.length} {i18n.t.cat_count} · {current.tip()}
						</span>

						<CatalogList entries={shown} onedit={(entry) => (editing = entry)} />
					{/if}

				</div>
			{/if}
			</ViewSwitch>
		</div>
	{/if}
</div>
