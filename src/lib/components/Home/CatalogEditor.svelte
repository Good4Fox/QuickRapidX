<script lang="ts">
	/*
	 * Создание и правка записи.
	 *
	 * Прежняя форма была набором полей без объяснений, а «ссылка на скачивание»
	 * и «код winget» лежали рядом как равные. Они не равны: у пакета winget
	 * скачивание идёт из проверенного манифеста, а прямая ссылка — это то, что
	 * вбили руками, и она устаревает. Поэтому идентификатор стоит первым и
	 * подписан, а ссылка идёт как запасной путь.
	 *
	 * Ещё здесь можно взять название и путь у программы, уже найденной на
	 * машине, — вместо того чтобы переписывать их вручную.
	 */
	import IsolationSettings from '$lib/components/Home/IsolationSettings.svelte';
	import type { AppEntry } from '$lib/data/appTypes';
	import { i18n } from '$lib/state/i18n.svelte';
	import { inventory } from '$lib/state/inventory.svelte';

	type Props = {
		entry: AppEntry;
		/** Правка существующей — тогда доступно удаление. */
		editing: boolean;
		onsave: (entry: AppEntry) => void;
		ondelete: () => void;
		oncancel: () => void;
	};

	let { entry, editing, onsave, ondelete, oncancel }: Props = $props();

	/*
	 * Черновик — копия: правка не должна менять запись в списке до сохранения.
	 * Пересобирается при смене записи, иначе форма показывала бы прежнюю после
	 * перехода к другой.
	 */
	let draft = $state<AppEntry>({
		id: '',
		site: '',
		download: '',
		download_plus: '',
		programm_ico: '',
		programm_name: '',
		programm_description: ''
	});

	// Пересобирается при смене записи: иначе форма показывала бы прежнюю
	// после перехода к другой
	$effect(() => {
		const source = entry;
		draft = { ...source };
	});
	let picking = $state(false);
	let query = $state('');

	const found = $derived.by(() => {
		const needle = query.trim().toLowerCase();
		if (!needle) return [];

		return inventory.findings
			.filter((finding) => finding.name.toLowerCase().includes(needle))
			.slice(0, 8);
	});

	$effect(() => {
		inventory.ensure();
	});

	const valid = $derived(draft.programm_name.trim().length > 0);

	/*
	 * Что предложить изоляции как файл программы.
	 *
	 * Поля «путь к исполняемому файлу» у записи нет: `programm_ico` хранит то, у
	 * чего берётся значок, а взятое с машины — это как раз exe. Значит,
	 * подсказка есть, а угадывать по картинке не приходится.
	 */
	const exe = $derived(draft.programm_ico.toLowerCase().endsWith('.exe') ? draft.programm_ico : '');
</script>

<div class="qrx_form">
	<header class="qrx_form_head">
		<span class="qrx_set_row_title">{editing ? i18n.t.cat_edit : i18n.t.cat_new}</span>

		<button class="qrx_rec_log" onclick={() => (picking = !picking)}>
			{i18n.t.cat_from_found}
		</button>
	</header>

	{#if picking}
		<!-- Название и путь можно взять у уже найденной программы -->
		<div class="qrx_form_pick">
			<input
				class="qrx_found_search"
				type="text"
				placeholder={i18n.t.found_search}
				bind:value={query}
				spellcheck="false"
			/>

			{#each found as finding (finding.key)}
				<button
					class="qrx_form_found"
					onclick={() => {
						draft.programm_name = finding.name;
						draft.programm_description = finding.publisher || finding.location;
						draft.programm_ico = finding.exe;
						picking = false;
						query = '';
					}}
				>
					{finding.name}
				</button>
			{/each}
		</div>
	{/if}

	<label class="qrx_field">
		<span class="qrx_field_label">{i18n.t.cat_name}</span>
		<input class="qrx_field_input" type="text" bind:value={draft.programm_name} />
	</label>

	<label class="qrx_field">
		<span class="qrx_field_label">{i18n.t.cat_desc}</span>
		<input class="qrx_field_input" type="text" bind:value={draft.programm_description} />
	</label>

	<label class="qrx_field">
		<span class="qrx_field_label">{i18n.t.cat_wg}</span>
		<input
			class="qrx_field_input"
			type="text"
			placeholder="Mozilla.Firefox"
			bind:value={draft.download_plus}
			spellcheck="false"
		/>
		<span class="qrx_rec_task_note">{i18n.t.cat_wg_hint}</span>
	</label>

	<label class="qrx_field">
		<span class="qrx_field_label">{i18n.t.cat_link}</span>
		<input
			class="qrx_field_input"
			type="text"
			placeholder="https://"
			bind:value={draft.download}
			spellcheck="false"
		/>
	</label>

	<label class="qrx_field">
		<span class="qrx_field_label">{i18n.t.cat_site}</span>
		<input
			class="qrx_field_input"
			type="text"
			placeholder="https://"
			bind:value={draft.site}
			spellcheck="false"
		/>
	</label>

	<!-- Изоляция сохраняется сама и сразу: это не поле записи, а поведение
	     запуска. Стоит после полей, чтобы не заслонять собой саму запись -->
	<IsolationSettings target={draft.id} name={draft.programm_name} suggested={exe} />

	<div class="qrx_form_actions">
		<button class="qrx_sys_link" disabled={!valid} onclick={() => onsave(draft)}>
			{i18n.t.cat_save}
		</button>

		<button class="qrx_rec_log" onclick={oncancel}>{i18n.t.cat_cancel}</button>

		{#if editing}
			<button class="qrx_rec_log qrx_lib_drop" onclick={ondelete}>{i18n.t.cat_delete}</button>
		{/if}
	</div>
</div>
