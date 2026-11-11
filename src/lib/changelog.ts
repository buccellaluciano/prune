/**
 * Historial de versiones que viene dentro de la app (funciona sin internet y con el repo
 * privado). La sección Versiones muestra las que son iguales o anteriores a la instalada;
 * una entrada nueva aparece sola al subir la versión. El script de release exige que la
 * versión que se publica tenga su entrada acá.
 */
import type { Lang } from "./i18n.svelte";

export interface Release {
  version: string;
  /** Fecha de publicación (AAAA-MM-DD). */
  date: string;
  notes: Record<Lang, string[]>;
}

export const CHANGELOG: Release[] = [
  {
    version: "0.5.0",
    date: "2026-11-11",
    notes: {
      es: [
        "Nueva sección Inicio de Windows: mirá qué arranca al iniciar sesión (registro, carpeta Inicio y tareas programadas), quién lo firma, y desactivalo como lo hace el Administrador de tareas, sin borrar nada.",
        "Punto de restauración antes de aplicar cambios grandes o quitar apps, y desde el Historial se crea uno o se abre Restaurar sistema.",
        "Windows Update: pausá las actualizaciones hasta 5 semanas, elegí las horas activas y evitá que instale drivers por su cuenta.",
        "Exportá tu configuración a un archivo e importala en otra PC: muestra qué cambiaría antes de aplicar nada.",
        "Si una actualización de Windows vuelve atrás algo que aplicaste o reinstala una app que quitaste, P-Rune te avisa para reaplicarlo o ignorarlo.",
      ],
      en: [
        "New Windows startup section: see what starts when you sign in (registry, Startup folder and scheduled tasks), who signed it, and turn it off the way Task Manager does, without deleting anything.",
        "Restore point before big changes or removing apps, and from History you can create one or open System Restore.",
        "Windows Update: pause updates for up to 5 weeks, choose active hours and stop it from installing drivers on its own.",
        "Export your configuration to a file and import it on another PC: it shows what would change before applying anything.",
        "If a Windows update reverts something you applied or reinstalls an app you removed, P-Rune lets you know so you can reapply it or ignore it.",
      ],
      "pt-BR": [
        "Nova seção Inicialização do Windows: veja o que abre ao entrar (registro, pasta Inicializar e tarefas agendadas), quem assinou, e desative como o Gerenciador de Tarefas faz, sem apagar nada.",
        "Ponto de restauração antes de grandes alterações ou de remover apps, e pelo Histórico você cria um ou abre a Restauração do Sistema.",
        "Windows Update: pause as atualizações por até 5 semanas, escolha o horário ativo e impeça que instale drivers por conta própria.",
        "Exporte sua configuração para um arquivo e importe em outro PC: mostra o que mudaria antes de aplicar qualquer coisa.",
        "Se uma atualização do Windows desfizer algo que você aplicou ou reinstalar um app que você removeu, o P-Rune avisa para reaplicar ou ignorar.",
      ],
      fr: [
        "Nouvelle section Démarrage de Windows : voyez ce qui se lance à l'ouverture de session (registre, dossier Démarrage et tâches planifiées), qui l'a signé, et désactivez-le comme le Gestionnaire des tâches, sans rien supprimer.",
        "Point de restauration avant les gros changements ou la suppression d'applications ; depuis l'Historique, créez-en un ou ouvrez la Restauration du système.",
        "Windows Update : suspendez les mises à jour jusqu'à 5 semaines, choisissez les heures d'activité et empêchez-le d'installer des pilotes tout seul.",
        "Exportez votre configuration dans un fichier et importez-la sur un autre PC : elle montre ce qui changerait avant d'appliquer quoi que ce soit.",
        "Si une mise à jour de Windows annule un réglage appliqué ou réinstalle une application supprimée, P-Rune vous prévient pour le réappliquer ou l'ignorer.",
      ],
      de: [
        "Neuer Bereich Windows-Autostart: Sieh, was bei der Anmeldung startet (Registry, Autostart-Ordner und geplante Aufgaben), wer es signiert hat, und deaktiviere es wie im Task-Manager, ohne etwas zu löschen.",
        "Wiederherstellungspunkt vor großen Änderungen oder dem Entfernen von Apps; im Verlauf kannst du einen erstellen oder die Systemwiederherstellung öffnen.",
        "Windows Update: Pausiere Updates bis zu 5 Wochen, wähle die Nutzungszeit und verhindere, dass es selbst Treiber installiert.",
        "Exportiere deine Konfiguration in eine Datei und importiere sie auf einem anderen PC: Es wird angezeigt, was sich ändern würde, bevor etwas angewendet wird.",
        "Wenn ein Windows-Update etwas Angewendetes zurücksetzt oder eine entfernte App wieder installiert, sagt P-Rune Bescheid, damit du es erneut anwenden oder ignorieren kannst.",
      ],
      it: [
        "Nuova sezione Avvio di Windows: vedi cosa si avvia all'accesso (registro, cartella Esecuzione automatica e attività pianificate), chi l'ha firmato, e disattivalo come fa Gestione attività, senza eliminare nulla.",
        "Punto di ripristino prima di grandi modifiche o della rimozione di app; dalla Cronologia puoi crearne uno o aprire Ripristino configurazione di sistema.",
        "Windows Update: sospendi gli aggiornamenti fino a 5 settimane, scegli l'orario di attività ed evita che installi driver da solo.",
        "Esporta la configurazione in un file e importala su un altro PC: mostra cosa cambierebbe prima di applicare qualsiasi cosa.",
        "Se un aggiornamento di Windows annulla qualcosa che hai applicato o reinstalla un'app che avevi rimosso, P-Rune ti avvisa per riapplicarlo o ignorarlo.",
      ],
      "zh-CN": [
        "新增“Windows 启动项”：查看登录时自动启动的内容（注册表、启动文件夹和计划任务）及其签名者，并像任务管理器一样禁用，不删除任何东西。",
        "在重大更改或移除应用之前创建还原点；也可以在“历史”中创建还原点或打开系统还原。",
        "Windows 更新：最多暂停 5 周、设置使用时段，并阻止其自行安装驱动程序。",
        "将配置导出到文件并在另一台电脑上导入：应用前会先显示会改什么。",
        "如果 Windows 更新撤销了你应用的设置或重新安装了你移除的应用，P-Rune 会提醒你重新应用或忽略。",
      ],
      ja: [
        "新しい「Windows スタートアップ」: サインイン時に起動するもの (レジストリ、スタートアップ フォルダー、タスク スケジューラ) と署名者を確認し、タスク マネージャーと同じ方法で何も削除せずに無効にできます。",
        "大きな変更やアプリの削除の前に復元ポイントを作成。履歴から作成したり、システムの復元を開いたりできます。",
        "Windows Update: 最大 5 週間の一時停止、アクティブ時間の設定、ドライバーの自動インストールの停止。",
        "構成をファイルにエクスポートし、別の PC でインポートできます。適用する前に何が変わるかを表示します。",
        "Windows の更新で適用した内容が元に戻ったり、削除したアプリが再インストールされたりすると、P-Rune が知らせ、再適用または無視を選べます。",
      ],
      ru: [
        "Новый раздел «Автозагрузка Windows»: что запускается при входе (реестр, папка автозагрузки и задачи планировщика), кто это подписал, и отключение, как в Диспетчере задач, без удаления.",
        "Точка восстановления перед крупными изменениями или удалением приложений; в «Истории» можно создать её или открыть восстановление системы.",
        "Центр обновления: пауза до 5 недель, выбор периода активности и запрет самостоятельной установки драйверов.",
        "Экспорт конфигурации в файл и импорт на другом ПК: перед применением показывается, что изменится.",
        "Если обновление Windows отменило применённое или вернуло удалённое приложение, P-Rune сообщит об этом, чтобы применить снова или игнорировать.",
      ],
    },
  },
  {
    version: "0.4.0",
    date: "2026-11-11",
    notes: {
      es: [
        "Nueva sección Red: elegí los DNS de cada conexión (IPv4 e IPv6) entre Cloudflare, Google, Quad9 y AdGuard, o cargalos a mano.",
        "DNS cifrado (DNS sobre HTTPS) para los proveedores de la lista, con cifrado preferido por defecto.",
        "Cada cambio de DNS se prueba antes y vuelve atrás solo si no confirmás en 15 segundos que internet sigue andando.",
        "Proxy: activá o desactivá el proxy manual, el script de configuración y la detección automática que ya tengas configurados.",
        "Los cambios de red quedan en el historial y se deshacen como cualquier otro.",
      ],
      en: [
        "New Network section: pick each connection's DNS (IPv4 and IPv6) from Cloudflare, Google, Quad9 and AdGuard, or enter them by hand.",
        "Encrypted DNS (DNS over HTTPS) for the providers in the list, with encrypted-preferred by default.",
        "Every DNS change is tested first and rolls back on its own unless you confirm within 15 seconds that the internet still works.",
        "Proxy: turn on or off the manual proxy, setup script and automatic detection you already have configured.",
        "Network changes are saved in the history and can be undone like any other.",
      ],
      "pt-BR": [
        "Nova seção Rede: escolha os DNS de cada conexão (IPv4 e IPv6) entre Cloudflare, Google, Quad9 e AdGuard, ou digite-os à mão.",
        "DNS criptografado (DNS sobre HTTPS) para os provedores da lista, com criptografia preferencial por padrão.",
        "Cada alteração de DNS é testada antes e volta sozinha se você não confirmar em 15 segundos que a internet continua funcionando.",
        "Proxy: ative ou desative o proxy manual, o script de configuração e a detecção automática que você já tem configurados.",
        "As alterações de rede ficam no histórico e podem ser desfeitas como qualquer outra.",
      ],
      fr: [
        "Nouvelle section Réseau : choisissez les DNS de chaque connexion (IPv4 et IPv6) parmi Cloudflare, Google, Quad9 et AdGuard, ou saisissez-les à la main.",
        "DNS chiffré (DNS sur HTTPS) pour les fournisseurs de la liste, chiffré de préférence par défaut.",
        "Chaque modification DNS est testée avant et annulée automatiquement si vous ne confirmez pas sous 15 secondes qu'Internet fonctionne.",
        "Proxy : activez ou désactivez le proxy manuel, le script de configuration et la détection automatique déjà configurés.",
        "Les modifications réseau sont enregistrées dans l'historique et s'annulent comme les autres.",
      ],
      de: [
        "Neuer Bereich Netzwerk: Wähle den DNS jeder Verbindung (IPv4 und IPv6) aus Cloudflare, Google, Quad9 und AdGuard oder gib ihn von Hand ein.",
        "Verschlüsselter DNS (DNS über HTTPS) für die Anbieter der Liste, standardmäßig „verschlüsselt bevorzugt“.",
        "Jede DNS-Änderung wird vorher getestet und automatisch zurückgenommen, wenn du nicht innerhalb von 15 Sekunden bestätigst, dass das Internet funktioniert.",
        "Proxy: Schalte den bereits eingerichteten manuellen Proxy, das Setupskript und die automatische Erkennung ein oder aus.",
        "Netzwerkänderungen landen im Verlauf und lassen sich wie alle anderen rückgängig machen.",
      ],
      it: [
        "Nuova sezione Rete: scegli i DNS di ogni connessione (IPv4 e IPv6) tra Cloudflare, Google, Quad9 e AdGuard, o inseriscili a mano.",
        "DNS crittografato (DNS su HTTPS) per i provider dell'elenco, con crittografia preferita per impostazione predefinita.",
        "Ogni modifica DNS viene provata prima e torna indietro da sola se non confermi entro 15 secondi che internet funziona ancora.",
        "Proxy: attiva o disattiva il proxy manuale, lo script di configurazione e il rilevamento automatico già configurati.",
        "Le modifiche di rete restano nella cronologia e si annullano come le altre.",
      ],
      "zh-CN": [
        "新增“网络”：为每个连接（IPv4 和 IPv6）从 Cloudflare、Google、Quad9 和 AdGuard 中选择 DNS，或手动填写。",
        "列表中的服务商支持加密 DNS（DNS over HTTPS），默认优先加密。",
        "每次更改 DNS 前都会先测试；如果 15 秒内未确认网络正常，会自动恢复。",
        "代理：开启或关闭已配置的手动代理、设置脚本和自动检测。",
        "网络更改会记入历史，可以像其他更改一样撤销。",
      ],
      ja: [
        "新しい「ネットワーク」: 接続ごとの DNS (IPv4 と IPv6) を Cloudflare、Google、Quad9、AdGuard から選ぶか、手動で入力できます。",
        "リストのプロバイダーで暗号化 DNS (DNS over HTTPS) に対応。既定は「暗号化を優先」。",
        "DNS の変更は事前にテストされ、15 秒以内にインターネットが使えると確認しないと自動で元に戻ります。",
        "プロキシ: 設定済みの手動プロキシ、セットアップ スクリプト、自動検出をオン/オフできます。",
        "ネットワークの変更は履歴に残り、ほかの変更と同じく元に戻せます。",
      ],
      ru: [
        "Новый раздел «Сеть»: выберите DNS для каждого подключения (IPv4 и IPv6) из Cloudflare, Google, Quad9 и AdGuard или введите вручную.",
        "Шифрованный DNS (DNS через HTTPS) для провайдеров из списка, по умолчанию — «предпочтительно шифровать».",
        "Каждое изменение DNS сначала проверяется и отменяется само, если в течение 15 секунд не подтвердить, что интернет работает.",
        "Прокси: включайте и выключайте уже настроенные ручной прокси, сценарий настройки и автоматическое определение.",
        "Изменения сети сохраняются в истории и отменяются, как и любые другие.",
      ],
    },
  },
  {
    version: "0.3.2",
    date: "2026-11-10",
    notes: {
      es: [
        "Arreglado: instalar WMO desde Más apps fallaba con \"respuesta demasiado grande\". Ahora instala la versión 1.1.0 actual (con su ícono nuevo).",
        "Si la descarga de WMO no coincide con la versión verificada, el aviso lo dice claramente.",
      ],
      en: [
        "Fixed: installing WMO from More apps failed with \"response too large\". It now installs the current 1.1.0 build (with its new icon).",
        "If the WMO download doesn't match the verified release, the message now says so clearly.",
      ],
      "pt-BR": [
        "Corrigido: instalar o WMO em Mais apps falhava com \"resposta grande demais\". Agora instala a versão 1.1.0 atual (com o ícone novo).",
        "Se o download do WMO não corresponder à versão verificada, o aviso agora diz isso claramente.",
      ],
      fr: [
        "Corrigé : l'installation de WMO depuis Plus d'apps échouait avec « réponse trop volumineuse ». Elle installe maintenant la version 1.1.0 actuelle (avec sa nouvelle icône).",
        "Si le téléchargement de WMO ne correspond pas à la version vérifiée, le message l'indique clairement.",
      ],
      de: [
        "Behoben: Die Installation von WMO über Weitere Apps schlug mit „Antwort zu groß“ fehl. Jetzt wird der aktuelle Build 1.1.0 (mit neuem Symbol) installiert.",
        "Wenn der WMO-Download nicht mit der geprüften Version übereinstimmt, sagt die Meldung das jetzt klar.",
      ],
      it: [
        "Risolto: l'installazione di WMO da Altre app falliva con \"risposta troppo grande\". Ora installa la versione 1.1.0 attuale (con la nuova icona).",
        "Se il download di WMO non corrisponde alla versione verificata, il messaggio ora lo dice chiaramente.",
      ],
      "zh-CN": [
        "已修复：从“更多应用”安装 WMO 时出现“响应过大”而失败。现在会安装当前的 1.1.0 版本（带新图标）。",
        "如果下载的 WMO 与经过验证的版本不一致，提示会清楚说明。",
      ],
      ja: [
        "修正: 「その他のアプリ」から WMO をインストールすると「応答が大きすぎます」で失敗していた問題。現在の 1.1.0 (新しいアイコン) をインストールします。",
        "WMO のダウンロードが検証済みのものと一致しない場合、そのことをわかりやすく表示します。",
      ],
      ru: [
        "Исправлено: установка WMO из раздела «Другие приложения» завершалась ошибкой «слишком большой ответ». Теперь устанавливается актуальная сборка 1.1.0 (с новым значком).",
        "Если загрузка WMO не совпадает с проверенной версией, сообщение теперь говорит об этом прямо.",
      ],
    },
  },
  {
    version: "0.3.1",
    date: "2026-11-10",
    notes: {
      es: [
        "Si no se puede consultar si hay actualizaciones (sin conexión o GitHub no responde), ahora lo dice con un mensaje claro en tu idioma.",
        "Los instaladores ya no incluyen rutas de la PC donde se compilan.",
        "Nueva sección Más apps: instalá WMO, una isla dinámica para controlar la música, verificada y sin permisos de administrador.",
        "Si un cambio falla al aplicarlo, queda pendiente para reintentarlo en vez de perderse.",
      ],
      en: [
        "If checking for updates isn't possible (offline, or GitHub doesn't respond), it now says so with a clear message in your language.",
        "The installers no longer include paths from the PC they were built on.",
        "New More apps section: install WMO, a dynamic island to control your music, verified and without administrator rights.",
        "If a change fails to apply, it stays pending so you can retry it instead of being lost.",
      ],
      "pt-BR": [
        "Se não for possível verificar se há atualizações (sem conexão ou o GitHub não responde), agora isso aparece com uma mensagem clara no seu idioma.",
        "Os instaladores não incluem mais caminhos do PC onde foram compilados.",
        "Nova seção Mais apps: instale o WMO, uma ilha dinâmica para controlar a música, verificado e sem permissões de administrador.",
        "Se uma alteração falhar ao ser aplicada, ela fica pendente para tentar de novo em vez de se perder.",
      ],
      fr: [
        "Si la recherche de mises à jour est impossible (hors connexion ou GitHub ne répond pas), un message clair l'indique désormais dans votre langue.",
        "Les installateurs n'incluent plus de chemins du PC sur lequel ils ont été compilés.",
        "Nouvelle section Plus d'apps : installez WMO, une île dynamique pour contrôler la musique, vérifiée et sans droits d'administrateur.",
        "Si une modification échoue, elle reste en attente pour pouvoir réessayer au lieu d'être perdue.",
      ],
      de: [
        "Wenn nicht nach Updates gesucht werden kann (offline oder GitHub antwortet nicht), erscheint jetzt eine klare Meldung in deiner Sprache.",
        "Die Installationsprogramme enthalten keine Pfade des PCs mehr, auf dem sie erstellt wurden.",
        "Neuer Bereich Weitere Apps: Installiere WMO, eine Dynamic Island zur Musiksteuerung – geprüft und ohne Administratorrechte.",
        "Wenn eine Änderung fehlschlägt, bleibt sie ausstehend, damit du sie erneut versuchen kannst, statt verloren zu gehen.",
      ],
      it: [
        "Se non è possibile cercare aggiornamenti (offline o GitHub non risponde), ora lo indica un messaggio chiaro nella tua lingua.",
        "Gli installer non includono più percorsi del PC su cui sono stati compilati.",
        "Nuova sezione Altre app: installa WMO, un'isola dinamica per controllare la musica, verificata e senza permessi di amministratore.",
        "Se una modifica non va a buon fine, resta in sospeso per riprovare invece di andare persa.",
      ],
      "zh-CN": [
        "无法检查更新时（没有网络或 GitHub 没有响应），现在会用你的语言给出清楚的提示。",
        "安装程序不再包含编译所用电脑上的路径。",
        "新增“更多应用”：安装 WMO（控制音乐的灵动岛），经过验证，无需管理员权限。",
        "某项更改应用失败时，会保留为待处理以便重试，而不会丢失。",
      ],
      ja: [
        "更新を確認できないとき（オフライン、または GitHub が応答しない場合）、お使いの言語でわかりやすく表示するようになりました。",
        "インストーラーに、ビルドした PC のパスが含まれなくなりました。",
        "新しい「その他のアプリ」: 音楽を操作するダイナミック アイランド WMO を、検証済みで管理者権限なしでインストールできます。",
        "変更の適用に失敗しても消えずに保留のまま残り、再試行できます。",
      ],
      ru: [
        "Если проверить обновления не удаётся (нет подключения или GitHub не отвечает), теперь об этом сообщается понятным сообщением на вашем языке.",
        "Установщики больше не содержат пути с компьютера, на котором они собраны.",
        "Новый раздел «Другие приложения»: установите WMO — динамический остров для управления музыкой, проверенный и без прав администратора.",
        "Если изменение не удалось применить, оно остаётся в ожидании для повтора, а не теряется.",
      ],
    },
  },
  {
    version: "0.3.0",
    date: "2026-11-08",
    notes: {
      es: [
        "Nueva categoría Servicios: desactivá servicios de Windows que no hacen falta, con el riesgo de cada uno.",
        "Limpieza en Disco: temporales, cachés, volcados, Windows Update, Spotify, Prefetch y papelera, con vista previa.",
        "Auto-actualización: P-Rune avisa cuando hay una versión nueva y se actualiza sola, verificando la firma.",
        "Antes de instalar una actualización, Windows te pide confirmar que sos vos.",
        "Nueva sección Versiones y mejor detección de programas instalados fuera de winget (como Brave).",
      ],
      en: [
        "New Services category: turn off Windows services you don't need, with the risk of each one.",
        "Cleanup in Disk: temp files, caches, dumps, Windows Update, Spotify, Prefetch and Recycle Bin, with a preview.",
        "Self-update: P-Rune tells you when there's a new version and updates itself, verifying the signature.",
        "Before installing an update, Windows asks you to confirm it's you.",
        "New Versions section and better detection of apps installed outside winget (like Brave).",
      ],
      "pt-BR": [
        "Nova categoria Serviços: desative serviços do Windows que não fazem falta, com o risco de cada um.",
        "Limpeza em Disco: temporários, caches, despejos, Windows Update, Spotify, Prefetch e lixeira, com prévia.",
        "Atualização automática: o P-Rune avisa quando há uma versão nova e se atualiza sozinho, verificando a assinatura.",
        "Antes de instalar uma atualização, o Windows pede que você confirme que é você.",
        "Nova seção Versões e melhor detecção de programas instalados fora do winget (como o Brave).",
      ],
      fr: [
        "Nouvelle catégorie Services : désactivez les services Windows inutiles, avec le risque de chacun.",
        "Nettoyage dans Disque : fichiers temporaires, caches, images mémoire, Windows Update, Spotify, Prefetch et corbeille, avec aperçu.",
        "Mise à jour automatique : P-Rune signale une nouvelle version et se met à jour seul, en vérifiant la signature.",
        "Avant d'installer une mise à jour, Windows vous demande de confirmer que c'est bien vous.",
        "Nouvelle section Versions et meilleure détection des programmes installés hors de winget (comme Brave).",
      ],
      de: [
        "Neue Kategorie Dienste: Schalte unnötige Windows-Dienste ab, mit dem Risiko jedes einzelnen.",
        "Bereinigung unter Datenträger: temporäre Dateien, Caches, Speicherabbilder, Windows Update, Spotify, Prefetch und Papierkorb, mit Vorschau.",
        "Selbstaktualisierung: P-Rune meldet neue Versionen und aktualisiert sich selbst, mit Signaturprüfung.",
        "Vor der Installation eines Updates bittet Windows dich, deine Identität zu bestätigen.",
        "Neuer Bereich Versionen und bessere Erkennung von Programmen, die außerhalb von winget installiert wurden (wie Brave).",
      ],
      it: [
        "Nuova categoria Servizi: disattiva i servizi di Windows che non servono, con il rischio di ciascuno.",
        "Pulizia in Disco: file temporanei, cache, dump, Windows Update, Spotify, Prefetch e cestino, con anteprima.",
        "Aggiornamento automatico: P-Rune avvisa quando c'è una nuova versione e si aggiorna da solo, verificando la firma.",
        "Prima di installare un aggiornamento, Windows ti chiede di confermare che sei tu.",
        "Nuova sezione Versioni e rilevamento migliore dei programmi installati fuori da winget (come Brave).",
      ],
      "zh-CN": [
        "新增“服务”分类：关闭不需要的 Windows 服务，并标明每项的风险。",
        "磁盘中新增清理功能：临时文件、缓存、转储、Windows 更新、Spotify、预读取和回收站，可先预览。",
        "自动更新：有新版本时 P-Rune 会提示，并在验证签名后自行更新。",
        "安装更新前，Windows 会要求你确认是本人操作。",
        "新增“版本”页面，并更好地识别在 winget 之外安装的程序（例如 Brave）。",
      ],
      ja: [
        "新しい「サービス」カテゴリ：不要な Windows サービスを、それぞれのリスクを確認しながらオフにできます。",
        "ディスクにクリーンアップを追加：一時ファイル、キャッシュ、ダンプ、Windows Update、Spotify、プリフェッチ、ごみ箱をプレビュー付きで削除。",
        "自動更新：新しいバージョンがあると P-Rune が知らせ、署名を確認して自分で更新します。",
        "更新をインストールする前に、Windows で本人確認を求めます。",
        "新しい「バージョン」セクションと、winget 以外でインストールしたプログラム（Brave など）の検出を改善。",
      ],
      ru: [
        "Новая категория «Службы»: отключайте ненужные службы Windows с пометкой о риске каждой.",
        "Очистка в разделе «Диск»: временные файлы, кэши, дампы, Центр обновления Windows, Spotify, Prefetch и корзина — с предпросмотром.",
        "Автообновление: P-Rune сообщает о новой версии и обновляется сам, проверяя подпись.",
        "Перед установкой обновления Windows просит подтвердить, что это вы.",
        "Новый раздел «Версии» и улучшенное определение программ, установленных не через winget (например, Brave).",
      ],
    },
  },
  {
    version: "0.2.0",
    date: "2026-11-07",
    notes: {
      es: [
        "La app pasa a llamarse P-Rune.",
        "9 idiomas: español, inglés, portugués, francés, alemán, italiano, chino, japonés y ruso.",
        "Pantalla de bienvenida con tu foto y tu usuario de Windows.",
        "Actualización de SvelteKit que cierra alertas de seguridad de dependencias.",
      ],
      en: [
        "The app is now called P-Rune.",
        "9 languages: Spanish, English, Portuguese, French, German, Italian, Chinese, Japanese and Russian.",
        "Welcome screen with your Windows photo and username.",
        "SvelteKit update that closes dependency security alerts.",
      ],
      "pt-BR": [
        "O app passa a se chamar P-Rune.",
        "9 idiomas: espanhol, inglês, português, francês, alemão, italiano, chinês, japonês e russo.",
        "Tela de boas-vindas com sua foto e seu usuário do Windows.",
        "Atualização do SvelteKit que fecha alertas de segurança de dependências.",
      ],
      fr: [
        "L'app s'appelle désormais P-Rune.",
        "9 langues : espagnol, anglais, portugais, français, allemand, italien, chinois, japonais et russe.",
        "Écran d'accueil avec votre photo et votre nom d'utilisateur Windows.",
        "Mise à jour de SvelteKit qui corrige des alertes de sécurité de dépendances.",
      ],
      de: [
        "Die App heißt jetzt P-Rune.",
        "9 Sprachen: Spanisch, Englisch, Portugiesisch, Französisch, Deutsch, Italienisch, Chinesisch, Japanisch und Russisch.",
        "Begrüßungsbildschirm mit deinem Windows-Foto und Benutzernamen.",
        "SvelteKit-Update, das Sicherheitswarnungen von Abhängigkeiten behebt.",
      ],
      it: [
        "L'app ora si chiama P-Rune.",
        "9 lingue: spagnolo, inglese, portoghese, francese, tedesco, italiano, cinese, giapponese e russo.",
        "Schermata di benvenuto con la tua foto e il tuo utente di Windows.",
        "Aggiornamento di SvelteKit che chiude avvisi di sicurezza delle dipendenze.",
      ],
      "zh-CN": [
        "应用改名为 P-Rune。",
        "支持 9 种语言：西班牙语、英语、葡萄牙语、法语、德语、意大利语、中文、日语和俄语。",
        "启动欢迎界面，显示你的 Windows 头像和用户名。",
        "升级 SvelteKit，修复依赖项的安全警报。",
      ],
      ja: [
        "アプリ名が P-Rune になりました。",
        "9 言語に対応：スペイン語、英語、ポルトガル語、フランス語、ドイツ語、イタリア語、中国語、日本語、ロシア語。",
        "Windows のアカウント画像とユーザー名を表示するようこそ画面。",
        "依存関係のセキュリティ警告に対応した SvelteKit の更新。",
      ],
      ru: [
        "Приложение теперь называется P-Rune.",
        "9 языков: испанский, английский, португальский, французский, немецкий, итальянский, китайский, японский и русский.",
        "Экран приветствия с вашим фото и именем пользователя Windows.",
        "Обновление SvelteKit, закрывающее предупреждения безопасности зависимостей.",
      ],
    },
  },
  {
    version: "0.1.0",
    date: "2026-11-07",
    notes: {
      es: [
        "Primera versión (como Prune).",
        "Tweaks de privacidad, IA, Inicio, búsqueda, barra de tareas, Explorador, apariencia, gaming y energía, con el estado real del registro.",
        "Plan de cambios antes de aplicar e historial con deshacer exacto.",
        "Bloatware, programas con winget (con revisión del instalador en VirusTotal) y análisis de uso de disco.",
        "Temas Moderno y Aero 7.",
      ],
      en: [
        "First version (as Prune).",
        "Privacy, AI, Start, search, taskbar, File Explorer, appearance, gaming and power tweaks, showing the actual registry state.",
        "Change plan before applying, and history with exact undo.",
        "Bloatware, apps via winget (with installer review on VirusTotal) and disk usage analysis.",
        "Modern and Aero 7 themes.",
      ],
      "pt-BR": [
        "Primeira versão (como Prune).",
        "Ajustes de privacidade, IA, Iniciar, pesquisa, barra de tarefas, Explorador, aparência, jogos e energia, com o estado real do registro.",
        "Plano de alterações antes de aplicar e histórico com desfazer exato.",
        "Bloatware, programas via winget (com revisão do instalador no VirusTotal) e análise de uso do disco.",
        "Temas Moderno e Aero 7.",
      ],
      fr: [
        "Première version (sous le nom Prune).",
        "Réglages de confidentialité, IA, Démarrer, recherche, barre des tâches, Explorateur, apparence, jeu et alimentation, avec l'état réel du registre.",
        "Plan des modifications avant d'appliquer et historique avec annulation exacte.",
        "Bloatware, programmes via winget (avec vérification de l'installateur sur VirusTotal) et analyse de l'espace disque.",
        "Thèmes Moderne et Aero 7.",
      ],
      de: [
        "Erste Version (als Prune).",
        "Optimierungen für Datenschutz, KI, Start, Suche, Taskleiste, Explorer, Darstellung, Gaming und Energie, mit dem echten Zustand der Registrierung.",
        "Änderungsplan vor dem Anwenden und Verlauf mit exaktem Rückgängigmachen.",
        "Bloatware, Programme über winget (mit Prüfung des Installationsprogramms auf VirusTotal) und Analyse der Speichernutzung.",
        "Designs Modern und Aero 7.",
      ],
      it: [
        "Prima versione (come Prune).",
        "Ottimizzazioni di privacy, IA, Start, ricerca, barra delle applicazioni, Esplora file, aspetto, gaming e alimentazione, con lo stato reale del registro.",
        "Piano delle modifiche prima di applicare e cronologia con annullamento esatto.",
        "Bloatware, programmi tramite winget (con controllo dell'installer su VirusTotal) e analisi dell'uso del disco.",
        "Temi Moderno e Aero 7.",
      ],
      "zh-CN": [
        "第一个版本（当时名为 Prune）。",
        "隐私、AI、开始菜单、搜索、任务栏、资源管理器、外观、游戏和电源方面的优化，并显示注册表的真实状态。",
        "应用前先查看更改计划，历史记录可精确撤销。",
        "预装软件清理、通过 winget 安装程序（可在 VirusTotal 上核对安装程序）以及磁盘占用分析。",
        "现代和 Aero 7 两种主题。",
      ],
      ja: [
        "最初のバージョン（Prune として）。",
        "プライバシー、AI、スタート、検索、タスクバー、エクスプローラー、外観、ゲーム、電源の調整。レジストリの実際の状態を表示。",
        "適用前の変更計画と、正確に元に戻せる履歴。",
        "不要なアプリの削除、winget によるプログラムのインストール（VirusTotal でインストーラーを確認）、ディスク使用量の分析。",
        "モダンと Aero 7 のテーマ。",
      ],
      ru: [
        "Первая версия (под названием Prune).",
        "Настройки конфиденциальности, ИИ, «Пуска», поиска, панели задач, Проводника, внешнего вида, игр и электропитания с реальным состоянием реестра.",
        "План изменений перед применением и журнал с точной отменой.",
        "Удаление лишних приложений, программы через winget (с проверкой установщика на VirusTotal) и анализ занятого места на диске.",
        "Темы «Современная» и Aero 7.",
      ],
    },
  },
];

/** Compara versiones semánticas simples ("0.10.0" > "0.9.1"). */
export function compareVersions(a: string, b: string): number {
  const pa = a.split(/[.+-]/).map((x) => Number.parseInt(x, 10) || 0);
  const pb = b.split(/[.+-]/).map((x) => Number.parseInt(x, 10) || 0);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const d = (pa[i] ?? 0) - (pb[i] ?? 0);
    if (d) return Math.sign(d);
  }
  return 0;
}
