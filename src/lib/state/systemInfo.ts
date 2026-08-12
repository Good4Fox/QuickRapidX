import { invoke } from '@tauri-apps/api/core';

import { i18n } from '$lib/state/i18n.svelte';

export type DiskInfo = {
	mount: string;
	total: number;
	free: number;
};

export type SystemInfo = {
	os: string;
	os_version: string;
	kernel: string;
	host: string;
	/** Сколько система работает без перезагрузки, секунды. */
	uptime: number;
	cpu: string;
	cpu_cores: number;
	memory_total: number;
	memory_used: number;
	disks: DiskInfo[];
	admin: boolean;
};

/** Спрашивает у ядра сводку о системе. */
export async function loadSystemInfo(): Promise<SystemInfo | null> {
	try {
		return await invoke<SystemInfo>('system_info');
	} catch (e) {
		console.error('не удалось получить сведения о системе:', e);
		return null;
	}
}

/**
 * Человекочитаемый размер из байтов.
 *
 * Единицы берутся из словаря. Прежде здесь стояли латинские «B/KB/GB», и
 * комментарий объяснял это тем, что «в корзине трея так же», — а в корзине как
 * раз стояли кириллические «Б/КБ/ГБ», и на соседних вкладках одного окна трея
 * можно было видеть «1.50 ГБ» и «8.0 GB» разом. Правило, которое описывал тот
 * комментарий, не соблюдалось нигде.
 *
 * Словарь читается прямо здесь, а не передаётся доводом: `i18n` живёт в файле
 * с рунами, и обращение к нему из обычной функции всё равно попадает в
 * зависимости того места, откуда её позвали, — при смене языка размеры
 * перерисуются сами.
 */
export function formatBytes(bytes: number): string {
	const units = [i18n.t.size_b, i18n.t.size_kb, i18n.t.size_mb, i18n.t.size_gb, i18n.t.size_tb];

	let value = bytes;
	let unit = 0;

	while (value >= 1024 && unit < units.length - 1) {
		value /= 1024;
		unit++;
	}

	return `${value.toFixed(unit === 0 ? 0 : 1)} ${units[unit]}`;
}

/**
 * Сколько система на ногах: «3 д 4 ч», «12 мин».
 *
 * Те же ключи, что и в панели трея: там эта строка собиралась из словаря, а
 * здесь стояли зашитые «d/h/m» — одна и та же величина в двух местах писалась
 * по-разному.
 */
export function formatUptime(seconds: number): string {
	const days = Math.floor(seconds / 86400);
	const hours = Math.floor((seconds % 86400) / 3600);
	const minutes = Math.floor((seconds % 3600) / 60);

	if (days > 0) return `${days} ${i18n.t.sys_d} ${hours} ${i18n.t.sys_h}`;
	if (hours > 0) return `${hours} ${i18n.t.sys_h} ${minutes} ${i18n.t.sys_m}`;

	return `${minutes} ${i18n.t.sys_m}`;
}
