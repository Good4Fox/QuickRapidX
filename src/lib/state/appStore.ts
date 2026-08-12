import { invoke } from '@tauri-apps/api/core';

import type { AppEntry } from '$lib/data/appTypes';

/**
 * Значения по умолчанию для незаполненных полей новой записи.
 * Перенесены из исходного приложения как есть.
 */
const DEFAULTS = {
	site: 'https://github.com/Good4Fox',
	download_plus: '',
	download: 'https://github.com/Good4Fox',
	programm_ico: '',
	programm_name: 'Good_Fox',
	// Пусто, а не слово-заглушка: прежнее «Text» оседало в записи навсегда и
	// показывалось подписью в списке. Пустое поле человек заполнит или не
	// заполнит, но неправды в нём не будет
	programm_description: ''
};

/** Пустая запись с новым идентификатором. */
export function blankEntry(): AppEntry {
	return {
		id: crypto.randomUUID(),
		site: '',
		download: '',
		download_plus: '',
		programm_ico: '',
		programm_name: '',
		programm_description: ''
	};
}

/**
 * Сохраняет новую запись в категорию.
 * Возвращает `true`, если ядро приняло её.
 */
export async function createEntry(category: string, entry: AppEntry): Promise<boolean> {
	try {
		await invoke('create_json_data_info', {
			lineneId: entry.id || crypto.randomUUID(),
			linenenewSite: entry.site || DEFAULTS.site,
			linenenewDownloadPlus: entry.download_plus || DEFAULTS.download_plus,
			linenenewDownload: entry.download || DEFAULTS.download,
			linenenewProgrammIco: entry.programm_ico || DEFAULTS.programm_ico,
			linenenewProgrammName: entry.programm_name || DEFAULTS.programm_name,
			linenenewProgrammDescription: entry.programm_description || DEFAULTS.programm_description,
			linenjname: category
		});

		return true;
	} catch (e) {
		console.error('не удалось создать запись:', e);
		return false;
	}
}
