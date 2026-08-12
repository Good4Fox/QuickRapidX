<script lang="ts">
	/*
	 * Система в панели трея.
	 *
	 * Заведена, чтобы панель перестала быть наполовину пустой: в нижнем ряду
	 * стояли четыре заглушки «?», а на главной — пустота, если ни один набор не
	 * отмечен для панели.
	 *
	 * Взято только то, что уже умеет ядро: сводка о машине и четыре действия с
	 * питанием. Ничего нового ради заполнения не выдумано — пустое место лучше
	 * выдуманного дела.
	 *
	 * Действия с питанием — удержанием. Панель стоит у самого края экрана, к ней
	 * ведёт узкая дорожка мимо часов и значков, и промахнуться в «выключить»
	 * тут проще, чем где-либо ещё.
	 */
	import { invoke } from '@tauri-apps/api/core';

	import HoldButton from '$lib/components/ui/HoldButton.svelte';
	import { i18n } from '$lib/state/i18n.svelte';
	import { formatBytes, loadSystemInfo, type SystemInfo } from '$lib/state/systemInfo';

	let info = $state<SystemInfo | null>(null);

	$effect(() => {
		read();

		/*
		 * Пока панель открыта, сводка подтягивается сама. Память и время работы
		 * меняются на глазах, а панель живёт секунды — обновлять её нажатием
		 * значило бы показывать позавчерашнее.
		 */
		const beat = setInterval(read, 3000);

		return () => clearInterval(beat);
	});

	async function read() {
		const value = await loadSystemInfo();

		if (value) info = value;
	}

	/** Системный диск: он же тот, на котором корзина и папка Windows. */
	const disk = $derived(info?.disks?.[0] ?? null);

	const memoryShare = $derived(
		info && info.memory_total > 0 ? info.memory_used / info.memory_total : 0
	);

	const diskShare = $derived(disk && disk.total > 0 ? (disk.total - disk.free) / disk.total : 0);

	/** Время работы словами: дни и часы, минуты — только пока нет часов. */
	const uptime = $derived.by(() => {
		const seconds = info?.uptime ?? 0;

		const days = Math.floor(seconds / 86400);
		const hours = Math.floor((seconds % 86400) / 3600);
		const minutes = Math.floor((seconds % 3600) / 60);

		if (days > 0) return `${days} ${i18n.t.sys_d} ${hours} ${i18n.t.sys_h}`;
		if (hours > 0) return `${hours} ${i18n.t.sys_h} ${minutes} ${i18n.t.sys_m}`;

		return `${minutes} ${i18n.t.sys_m}`;
	});

	/** Те же четыре действия и в том же порядке, что в настройках Windows. */
	const POWER = $derived([
		{ ico: 'qrx_sys_ico_lock', label: i18n.t.sys_lock, run: () => invoke('windows_lock_workstation') },
		{ ico: 'qrx_sys_ico_sleep', label: i18n.t.sys_sleep, run: () => invoke('windows_sleep') },
		{ ico: 'qrx_sys_ico_restart', label: i18n.t.sys_restart, run: () => invoke('windows_restart') },
		{ ico: 'qrx_sys_ico_power', label: i18n.t.sys_shutdown, run: () => invoke('windows_shutdown') }
	]);
</script>

<div class="tray_sys">
	<!-- Сводка: три строки, каждая с полосой. Числа без полосы в панели
	     читаются дольше, чем она живёт на экране -->
	<div class="tray_sys_facts">
		<div class="tray_sys_fact">
			<span class="tray_sys_fact_name">{i18n.t.sys_memory}</span>
			<div class="tray_sys_bar">
				<div class="tray_sys_bar_fill" style="width: {memoryShare * 100}%"></div>
			</div>
			<span class="tray_sys_fact_value">
				{info ? `${formatBytes(info.memory_used)} / ${formatBytes(info.memory_total)}` : '…'}
			</span>
		</div>

		<div class="tray_sys_fact">
			<span class="tray_sys_fact_name">{disk?.mount ?? i18n.t.sys_disk}</span>
			<div class="tray_sys_bar">
				<div class="tray_sys_bar_fill" style="width: {diskShare * 100}%"></div>
			</div>
			<span class="tray_sys_fact_value">
				{disk ? `${formatBytes(disk.free)} ${i18n.t.sys_free}` : '…'}
			</span>
		</div>

		<div class="tray_sys_fact tray_sys_fact_plain">
			<span class="tray_sys_fact_name">{i18n.t.sys_uptime}</span>
			<span class="tray_sys_fact_value">{info ? uptime : '…'}</span>
		</div>
	</div>

	<!-- Питание: два на два. В строку из четырёх подписи не помещаются, а без
	     подписей значки сна и выключения различают не все -->
	<div class="tray_sys_power">
		{#each POWER as action (action.ico)}
			<HoldButton
				compact
				label={action.label}
				ico={action.ico}
				hint={i18n.t.sys_hold}
				onrun={action.run}
			/>
		{/each}
	</div>
</div>
