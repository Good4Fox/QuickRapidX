/**
 * Подборка для витрины.
 *
 * winget не умеет отдавать «популярное»: у него есть только поиск по строке.
 * Поэтому список того, что показывается сразу, — наш: несколько десятков
 * пакетов, разложенных по назначению, чтобы не заставлять человека угадывать
 * названия.
 *
 * Идентификаторы выверены вручную и помечаются как проверенные. В winget
 * встречаются пакеты, выложенные посторонними, — у них то же название, но
 * другой издатель и другой источник загрузки. Для всего, что найдено поиском,
 * такой пометки нет, зато показывается домен, откуда качается установщик:
 * это и есть проверяемый признак, в отличие от названия.
 */
export type StorePick = { id: string; name: string; group: string };

export const STORE_GROUPS = [
	'browsers',
	'dev',
	'media',
	'chat',
	'tools',
	'archive'
] as const;

export type StoreGroup = (typeof STORE_GROUPS)[number];

export const STORE_PICKS: StorePick[] = [
	{ id: 'Mozilla.Firefox', name: 'Firefox', group: 'browsers' },
	{ id: 'Google.Chrome', name: 'Google Chrome', group: 'browsers' },
	{ id: 'Brave.Brave', name: 'Brave', group: 'browsers' },
	{ id: 'Yandex.Browser', name: 'Яндекс Браузер', group: 'browsers' },

	{ id: 'Microsoft.VisualStudioCode', name: 'Visual Studio Code', group: 'dev' },
	{ id: 'Git.Git', name: 'Git', group: 'dev' },
	{ id: 'OpenJS.NodeJS', name: 'Node.js', group: 'dev' },
	{ id: 'Rustlang.Rustup', name: 'Rustup', group: 'dev' },
	{ id: 'Python.Python.3.13', name: 'Python 3.13', group: 'dev' },
	{ id: 'Oven-sh.Bun', name: 'Bun', group: 'dev' },
	{ id: 'Docker.DockerDesktop', name: 'Docker Desktop', group: 'dev' },
	{ id: 'Microsoft.PowerShell', name: 'PowerShell', group: 'dev' },
	{ id: 'JetBrains.Toolbox', name: 'JetBrains Toolbox', group: 'dev' },

	{ id: 'VideoLAN.VLC', name: 'VLC', group: 'media' },
	{ id: 'clsid2.mpc-hc', name: 'MPC-HC', group: 'media' },
	{ id: 'OBSProject.OBSStudio', name: 'OBS Studio', group: 'media' },
	{ id: 'Audacity.Audacity', name: 'Audacity', group: 'media' },
	{ id: 'GIMP.GIMP.3', name: 'GIMP', group: 'media' },
	{ id: 'BlenderFoundation.Blender', name: 'Blender', group: 'media' },

	{ id: 'Telegram.TelegramDesktop', name: 'Telegram', group: 'chat' },
	{ id: 'Discord.Discord', name: 'Discord', group: 'chat' },
	{ id: 'Zoom.Zoom', name: 'Zoom', group: 'chat' },

	{ id: 'Microsoft.PowerToys', name: 'PowerToys', group: 'tools' },
	{ id: 'Microsoft.WindowsTerminal', name: 'Windows Terminal', group: 'tools' },
	{ id: 'Notepad++.Notepad++', name: 'Notepad++', group: 'tools' },
	{ id: 'voidtools.Everything', name: 'Everything', group: 'tools' },
	{ id: 'CrystalDewWorld.CrystalDiskInfo', name: 'CrystalDiskInfo', group: 'tools' },
	{ id: 'REALiX.HWiNFO', name: 'HWiNFO', group: 'tools' },
	{ id: 'AutoHotkey.AutoHotkey', name: 'AutoHotkey', group: 'tools' },

	{ id: '7zip.7zip', name: '7-Zip', group: 'archive' },
	{ id: 'RARLab.WinRAR', name: 'WinRAR', group: 'archive' },
	{ id: 'M2Team.NanaZip', name: 'NanaZip', group: 'archive' }
];
