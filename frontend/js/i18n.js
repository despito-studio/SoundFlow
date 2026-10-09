/**
 * SoundFlow Internationalization (i18n) Engine
 * Supported languages:
 *  - 'uk': Українська (Default)
 *  - 'ru': Русский
 *  - 'en': English
 *  - 'es': Español
 */

const SOUNDFLOW_TRANSLATIONS = {
    uk: {
        code: 'УКР',
        flag: '🇺🇦',
        name: 'Українська',

        // Navigation
        nav_home: 'Головна',
        nav_search: 'Пошук',
        nav_liked: 'Улюблені',
        nav_history: 'Історія',
        nav_profile: 'Профіль',
        nav_playlists_title: 'ПЛЕЙЛИСТИ',
        btn_create_playlist: 'Створити плейлист',

        // Topbar Search & Filter
        search_placeholder: 'Пошук музики, виконавців або міксів...',
        source_all: 'Всі',
        source_sc: 'SoundCloud',
        source_yt: 'YouTube',

        // Genre chips
        genre_lofi: '☕ Lo-Fi Chill',
        genre_synthwave: '🌆 Synthwave',
        genre_phonk: '🏎️ Phonk Drift',
        genre_deephouse: '🎧 Deep House',
        genre_rock: '🎸 Rock',
        genre_gaming: '🎮 Gaming Mix',
        genre_ambient: '🌿 Ambient',
        genre_ua: '🇺🇦 Українське',

        // Home View
        hero_tag: 'НОВИЙ РІВЕНЬ ЗВУКУ',
        hero_title: 'Музика без меж',
        hero_desc: 'Миттєвий онлайн-стрімінг треків із SoundCloud та YouTube. Жодних гальм, нуль зайвого навантаження.',
        hero_btn_mix: 'Увімкнути мікс',
        hero_btn_search: 'Знайти трек',
        section_popular: 'Популярні треки',

        // Liked View
        liked_header_tag: 'Ваша колекція',
        liked_header_title: 'Улюблені треки',
        liked_header_meta: 'треків у колекції',
        btn_play_all: 'Увімкнути всі',
        btn_shuffle: 'Перемішати',
        empty_liked_title: 'У вас поки немає збережених треків',
        empty_liked_desc: 'Натисніть сердечко на будь-якому треку, щоб додати його сюди.',

        // History View
        history_header_tag: 'Останні прослуховування',
        history_header_title: 'Історія відтворень',
        btn_clear_history: 'Очистити історію',
        empty_history_title: 'Історія прослуховувань порожня',
        empty_history_desc: 'Слухайте музику, і ваші останні треки з\'являться тут.',

        // Search View
        search_results_for: 'Результати пошуку для:',
        search_count_found: 'треків знайдено',
        search_empty_title: 'Нічого не знайдено',
        search_empty_desc: 'Спробуйте інший пошуковий запит або змініть джерело пошуку.',
        search_initial_title: 'Введіть назву треку або виконавця для пошуку',

        // SoundCloud Profile
        profile_edit_btn: 'Редагувати профіль',
        profile_banner_btn: 'Змінити банер',
        profile_badge_tooltip: 'Натисніть для вибору ролі',
        role_selector_title: 'Оберіть вашу роль:',
        profile_tab_all: 'Всі',
        profile_tab_popular: 'Популярні треки',
        profile_tab_liked: 'Улюблені',
        profile_tab_playlists: 'Плейлисти',
        profile_tab_history: 'Історія',
        profile_tab_admin: 'Панель адміністратора',
        profile_tab_friends: 'Друзі',

        // Friends
        friends_section_title: 'Мої друзі в SoundFlow',
        friends_section_subtitle: 'Слухайте музику разом, діліться треками та додавайте нових друзів.',
        btn_add_friend: 'Додати в друзі',
        modal_add_friend_title: 'Додати нового друга',
        modal_add_friend_desc: 'Введіть нікнейм або тег користувача SoundFlow:',
        friends_listening_to: 'Слухає',
        btn_listen_together: 'Слухати',
        sidebar_friends_header: 'ДРУЗІ',

        // Profile Roles
        role_admin: 'Адміністратор',
        role_moderator: 'Модератор',
        role_artist: 'Виконавець',
        role_listener: 'Слухач',

        // Profile Sidebar Stats
        stat_followers: 'Читачі',
        stat_following: 'Відстежує',
        stat_tracks: 'Треки',
        sidebar_likes_header: 'LIKES',
        sidebar_view_all: 'Переглянути всі',
        sidebar_empty_likes: 'Ще немає вподобаних треків',

        // Admin Panel
        admin_hero_pill: '👑 PRIVILEGES LEVEL: OWNER / ADMIN',
        admin_hero_status: '● СИСТЕМА АКТИВНА',
        admin_hero_title: 'Панель управління SoundFlow',
        admin_hero_desc: 'Повний доступ до аудіопроцесора DSP, бази даних та розширених налаштувань SoundFlow.',
        admin_card_hq_title: 'Студійне HQ Аудіо',
        admin_card_hq_desc: 'Максимальний бітрейт 320 kbps без компресії та артефактів.',
        admin_card_hq_active: 'АКТИВОВАНО (320 KBPS)',
        admin_card_bass_title: 'Студійний Bass Boost (DSP)',
        admin_card_bass_desc: 'Апаратний low-shelf фільтр для потужного глибокого басу.',
        admin_card_role_title: 'Перемикання статусу та ролі',
        admin_card_role_desc: 'Оберіть активну роль для вашого акаунта:',
        admin_card_banner_title: 'Оформлення та Теми Банера',
        admin_card_banner_desc: 'Оберіть пресет або налаштуйте власний банер профілю:',
        preset_beige: 'Beige Aesthetic',
        preset_neon: 'Neon Sunset',
        preset_cyber: 'Cyberpunk',
        preset_midnight: 'Midnight',
        admin_card_cache_title: 'Очищення та Оптимізація',
        admin_card_cache_desc: 'Миттєве звільнення кешу запитів, RAM та тимчасових файлів.',
        btn_clean_cache: 'Очистити кеш та RAM',
        admin_card_backup_title: 'Резервне копіювання',
        admin_card_backup_desc: 'Експорт або імпорт усієї медіатеки та налаштувань у форматі JSON.',
        btn_export_lib: 'Експорт медіатеки',
        btn_import_lib: 'Імпорт медіатеки',

        // Player Bar
        player_track_placeholder: 'Виберіть трек для відтворення',
        player_artist_placeholder: 'SoundFlow',
        queue_drawer_title: 'Черга відтворення',
        queue_now_playing: 'Зараз грає',
        queue_next_up: 'Далі у черзі',
        queue_empty_now: 'Нічого не грає',
        queue_empty_next: 'Кінець черги',
        btn_autoplay: 'Автоматичне радіо схожих треків',
        toast_autoplay_on: 'Авторадіо увімкнено: схожі треки вмикатимуться автоматично ♾️',
        toast_autoplay_off: 'Авторадіо вимкнено',
        toast_finding_similar: 'Підбираємо схожі пісні за тематикою... 🎵',
        queue_similar_badge: 'Схожі за темою',

        // Modals
        modal_create_pl_title: 'Створити новий плейлист',
        modal_create_pl_placeholder: 'Назва плейлиста...',
        modal_btn_cancel: 'Скасувати',
        modal_btn_create: 'Створити',
        modal_picker_pl_title: 'Додати до плейлиста',

        // Auth & User Menu
        btn_sign_in: 'Увійти',
        menu_view_profile: 'Мій профіль SoundCloud',
        menu_account_settings: 'Налаштування акаунта',
        menu_sign_out: 'Вийти',

        // Toasts
        toast_signed_out: 'Ви вийшли з акаунта',
        toast_welcome: 'Вітаємо',
        toast_liked: 'Додано до улюблених ❤️',
        toast_unliked: 'Видалено з улюблених',
        toast_history_cleared: 'Історію очищено',
        toast_cache_cleared: 'Кеш очищено, пам\'ять звільнено!',
        toast_role_switched: 'Роль змінено на',
        toast_theme_applied: 'Тему банера застосовано!',
        toast_backup_exported: 'Бібліотеку успішно збережено в файл',
        toast_backup_imported: 'Бібліотеку успішно імпортовано!'
    },

    ru: {
        code: 'РУС',
        flag: '🇷🇺',
        name: 'Русский',

        nav_home: 'Главная',
        nav_search: 'Поиск',
        nav_liked: 'Любимые',
        nav_history: 'История',
        nav_profile: 'Профиль',
        nav_playlists_title: 'ПЛЕЙЛИСТЫ',
        btn_create_playlist: 'Создать плейлист',

        search_placeholder: 'Поиск музыки, исполнителей или миксов...',
        source_all: 'Все',
        source_sc: 'SoundCloud',
        source_yt: 'YouTube',

        genre_lofi: '☕ Lo-Fi Chill',
        genre_synthwave: '🌆 Synthwave',
        genre_phonk: '🏎️ Phonk Drift',
        genre_deephouse: '🎧 Deep House',
        genre_rock: '🎸 Rock',
        genre_gaming: '🎮 Gaming Mix',
        genre_ambient: '🌿 Ambient',
        genre_ua: '🇺🇦 Украинское',

        hero_tag: 'НОВЫЙ УРОВЕНЬ ЗВУКА',
        hero_title: 'Музыка без границ',
        hero_desc: 'Мгновенный онлайн-стриминг треков из SoundCloud и YouTube. Никаких лагов, ноль лишней нагрузки.',
        hero_btn_mix: 'Включить микс',
        hero_btn_search: 'Найти трек',
        section_popular: 'Популярные треки',

        liked_header_tag: 'Ваша коллекция',
        liked_header_title: 'Любимые треки',
        liked_header_meta: 'треков в коллекции',
        btn_play_all: 'Включить все',
        btn_shuffle: 'Перемешать',
        empty_liked_title: 'У вас пока нет сохраненных треков',
        empty_liked_desc: 'Нажмите сердечко на любом треке, чтобы добавить его сюда.',

        history_header_tag: 'Недавние прослушивания',
        history_header_title: 'История воспроизведений',
        btn_clear_history: 'Очистить историю',
        empty_history_title: 'История прослушиваний пуста',
        empty_history_desc: 'Слушайте музыку, и ваши последние треки появятся здесь.',

        search_results_for: 'Результаты поиска для:',
        search_count_found: 'треков найдено',
        search_empty_title: 'Ничего не найдено',
        search_empty_desc: 'Попробуйте другой поисковый запрос или измените источник.',
        search_initial_title: 'Введите название трека или исполнителя для поиска',

        profile_edit_btn: 'Редактировать профиль',
        profile_banner_btn: 'Изменить баннер',
        profile_badge_tooltip: 'Нажмите для выбора роли',
        role_selector_title: 'Выберите вашу роль:',
        profile_tab_all: 'Все',
        profile_tab_popular: 'Популярные треки',
        profile_tab_liked: 'Любимые',
        profile_tab_playlists: 'Плейлисты',
        profile_tab_history: 'История',
        profile_tab_admin: 'Панель администратора',
        profile_tab_friends: 'Друзья',

        // Friends
        friends_section_title: 'Мои друзья в SoundFlow',
        friends_section_subtitle: 'Слушайте музыку вместе, делитесь треками и добавляйте новых друзей.',
        btn_add_friend: 'Добавить в друзья',
        modal_add_friend_title: 'Добавить нового друга',
        modal_add_friend_desc: 'Введите никнейм или тег пользователя SoundFlow:',
        friends_listening_to: 'Слушает',
        btn_listen_together: 'Слушать',
        sidebar_friends_header: 'ДРУЗЬЯ',

        role_admin: 'Администратор',
        role_moderator: 'Модератор',
        role_artist: 'Исполнитель',
        role_listener: 'Слушатель',

        stat_followers: 'Подписчики',
        stat_following: 'Подписки',
        stat_tracks: 'Треки',
        sidebar_likes_header: 'LIKES',
        sidebar_view_all: 'Посмотреть все',
        sidebar_following_header: 'FOLLOWING',
        sidebar_empty_likes: 'Еще нет понравившихся треков',

        admin_hero_pill: '👑 PRIVILEGES LEVEL: OWNER / ADMIN',
        admin_hero_status: '● СИСТЕМА АКТИВНА',
        admin_hero_title: 'Панель управления SoundFlow',
        admin_hero_desc: 'Полный доступ к аудиопроцессору DSP, базе данных и расширенным настройкам SoundFlow.',
        admin_card_hq_title: 'Студийное HQ Аудио',
        admin_card_hq_desc: 'Максимальный битрейт 320 kbps без сжатия и артефактов.',
        admin_card_hq_active: 'АКТИВИРОВАНО (320 KBPS)',
        admin_card_bass_title: 'Студийный Bass Boost (DSP)',
        admin_card_bass_desc: 'Аппаратный low-shelf фильтр для мощного глубокого баса.',
        admin_card_role_title: 'Переключение статуса и роли',
        admin_card_role_desc: 'Выберите активную роль для вашего аккаунта:',
        admin_card_banner_title: 'Оформление и Темы Баннера',
        admin_card_banner_desc: 'Выберите пресет или настройте собственный баннер профиля:',
        preset_beige: 'Beige Aesthetic',
        preset_neon: 'Neon Sunset',
        preset_cyber: 'Cyberpunk',
        preset_midnight: 'Midnight',
        admin_card_cache_title: 'Очистка и Оптимизация',
        admin_card_cache_desc: 'Мгновенное освобождение кэша запросов, RAM и временных файлов.',
        btn_clean_cache: 'Очистить кэш и RAM',
        admin_card_backup_title: 'Резервное копирование',
        admin_card_backup_desc: 'Экспорт или импорт всей медиатеки и настроек в формате JSON.',
        btn_export_lib: 'Экспорт медиатеки',
        btn_import_lib: 'Импорт медиатеки',

        player_track_placeholder: 'Выберите трек для воспроизведения',
        player_artist_placeholder: 'SoundFlow',
        queue_drawer_title: 'Очередь воспроизведения',
        queue_now_playing: 'Сейчас играет',
        queue_next_up: 'Далее в очереди',
        queue_empty_now: 'Ничего не играет',
        queue_empty_next: 'Конец очереди',
        btn_autoplay: 'Автоматическое радио похожих треков',
        toast_autoplay_on: 'Авторадио включено: похожие треки будут включаться автоматически ♾️',
        toast_autoplay_off: 'Авторадио выключено',
        toast_finding_similar: 'Подбираем похожие треки по тематике... 🎵',
        queue_similar_badge: 'Похожие по теме',

        modal_create_pl_title: 'Создать новый плейлист',
        modal_create_pl_placeholder: 'Название плейлиста...',
        modal_btn_cancel: 'Отмена',
        modal_btn_create: 'Создать',
        modal_picker_pl_title: 'Добавить в плейлист',

        btn_sign_in: 'Войти',
        menu_view_profile: 'Мой профиль SoundCloud',
        menu_account_settings: 'Настройки аккаунта',
        menu_sign_out: 'Выйти',

        toast_signed_out: 'Вы вышли из аккаунта',
        toast_welcome: 'Добро пожаловать',
        toast_liked: 'Добавлено в любимые ❤️',
        toast_unliked: 'Удалено из любимых',
        toast_history_cleared: 'История очищена',
        toast_cache_cleared: 'Кэш очищен, память освобождена!',
        toast_role_switched: 'Роль изменена на',
        toast_theme_applied: 'Тема баннера применена!',
        toast_backup_exported: 'Библиотека успешно сохранена в файл',
        toast_backup_imported: 'Библиотека успешно импортирована!'
    },

    en: {
        code: 'ENG',
        flag: '🇬🇧',
        name: 'English',

        nav_home: 'Home',
        nav_search: 'Search',
        nav_liked: 'Favorites',
        nav_history: 'History',
        nav_profile: 'Profile',
        nav_playlists_title: 'PLAYLISTS',
        btn_create_playlist: 'Create Playlist',

        search_placeholder: 'Search music, artists or mixes...',
        source_all: 'All',
        source_sc: 'SoundCloud',
        source_yt: 'YouTube',

        genre_lofi: '☕ Lo-Fi Chill',
        genre_synthwave: '🌆 Synthwave',
        genre_phonk: '🏎️ Phonk Drift',
        genre_deephouse: '🎧 Deep House',
        genre_rock: '🎸 Rock',
        genre_gaming: '🎮 Gaming Mix',
        genre_ambient: '🌿 Ambient',
        genre_ua: '🇺🇦 Ukrainian Hits',

        hero_tag: 'NEXT LEVEL SOUND',
        hero_title: 'Music Without Limits',
        hero_desc: 'Instant online streaming from SoundCloud and YouTube. Zero lag, ultra-light footprint.',
        hero_btn_mix: 'Play Mix',
        hero_btn_search: 'Find Track',
        section_popular: 'Popular Tracks',

        liked_header_tag: 'Your Collection',
        liked_header_title: 'Favorite Tracks',
        liked_header_meta: 'tracks in collection',
        btn_play_all: 'Play All',
        btn_shuffle: 'Shuffle',
        empty_liked_title: 'No saved tracks yet',
        empty_liked_desc: 'Click the heart icon on any track to save it here.',

        history_header_tag: 'Recent Plays',
        history_header_title: 'Listening History',
        btn_clear_history: 'Clear History',
        empty_history_title: 'Playback history is empty',
        empty_history_desc: 'Listen to tracks and your recent songs will appear here.',

        search_results_for: 'Search results for:',
        search_count_found: 'tracks found',
        search_empty_title: 'Nothing found',
        search_empty_desc: 'Try another search term or switch the source provider.',
        search_initial_title: 'Enter track title or artist name to search',

        profile_edit_btn: 'Edit Profile',
        profile_banner_btn: 'Change Banner',
        profile_badge_tooltip: 'Click to select role',
        role_selector_title: 'Select your role:',
        profile_tab_all: 'All',
        profile_tab_popular: 'Popular Tracks',
        profile_tab_liked: 'Favorites',
        profile_tab_playlists: 'Playlists',
        profile_tab_history: 'History',
        profile_tab_admin: 'Admin Panel',
        profile_tab_friends: 'Friends',

        // Friends
        friends_section_title: 'My Friends on SoundFlow',
        friends_section_subtitle: 'Listen to music together, share tracks, and connect with new friends.',
        btn_add_friend: 'Add Friend',
        modal_add_friend_title: 'Add New Friend',
        modal_add_friend_desc: 'Enter SoundFlow username or tag:',
        friends_listening_to: 'Listening to',
        btn_listen_together: 'Listen',
        sidebar_friends_header: 'FRIENDS',

        role_admin: 'Administrator',
        role_moderator: 'Moderator',
        role_artist: 'Artist',
        role_listener: 'Listener',

        stat_followers: 'Followers',
        stat_following: 'Following',
        stat_tracks: 'Tracks',
        sidebar_likes_header: 'LIKES',
        sidebar_view_all: 'View all',
        sidebar_following_header: 'FOLLOWING',
        sidebar_empty_likes: 'No liked tracks yet',

        admin_hero_pill: '👑 PRIVILEGES LEVEL: OWNER / ADMIN',
        admin_hero_status: '● SYSTEM ACTIVE',
        admin_hero_title: 'SoundFlow Control Panel',
        admin_hero_desc: 'Full access to WebAudio DSP hardware filter, database and advanced configurations.',
        admin_card_hq_title: 'Studio HQ Audio',
        admin_card_hq_desc: 'Maximum 320 kbps bitrate uncompressed audio stream.',
        admin_card_hq_active: 'ENABLED (320 KBPS)',
        admin_card_bass_title: 'Studio Bass Boost (DSP)',
        admin_card_bass_desc: 'Hardware Biquad low-shelf filter for deep studio-grade punchy bass.',
        admin_card_role_title: 'Role & Status Switcher',
        admin_card_role_desc: 'Select active role badge for your account:',
        admin_card_banner_title: 'Banner Styling & Themes',
        admin_card_banner_desc: 'Choose a preset or customize your profile header banner:',
        preset_beige: 'Beige Aesthetic',
        preset_neon: 'Neon Sunset',
        preset_cyber: 'Cyberpunk',
        preset_midnight: 'Midnight',
        admin_card_cache_title: 'Cleaning & Optimization',
        admin_card_cache_desc: 'Instantly purge search cache, RAM footprint and temp buffers.',
        btn_clean_cache: 'Clear Cache & RAM',
        admin_card_backup_title: 'Backup & Restore',
        admin_card_backup_desc: 'Export or import your entire library and preferences as JSON.',
        btn_export_lib: 'Export Library',
        btn_import_lib: 'Import Library',

        player_track_placeholder: 'Select a track to play',
        player_artist_placeholder: 'SoundFlow',
        queue_drawer_title: 'Playback Queue',
        queue_now_playing: 'Now Playing',
        queue_next_up: 'Next in Queue',
        queue_empty_now: 'Nothing playing',
        queue_empty_next: 'End of queue',
        btn_autoplay: 'Autoplay Similar Music',
        toast_autoplay_on: 'Autoplay enabled: similar tracks will play continuously ♾️',
        toast_autoplay_off: 'Autoplay disabled',
        toast_finding_similar: 'Finding similar music... 🎵',
        queue_similar_badge: 'Similar Theme',

        modal_create_pl_title: 'Create New Playlist',
        modal_create_pl_placeholder: 'Playlist title...',
        modal_btn_cancel: 'Cancel',
        modal_btn_create: 'Create',
        modal_picker_pl_title: 'Add to Playlist',

        btn_sign_in: 'Sign In',
        menu_view_profile: 'My SoundCloud Profile',
        menu_account_settings: 'Account Settings',
        menu_sign_out: 'Sign Out',

        toast_signed_out: 'You have signed out',
        toast_welcome: 'Welcome',
        toast_liked: 'Added to favorites ❤️',
        toast_unliked: 'Removed from favorites',
        toast_history_cleared: 'History cleared',
        toast_cache_cleared: 'Cache cleared and RAM optimized!',
        toast_role_switched: 'Role switched to',
        toast_theme_applied: 'Banner theme applied!',
        toast_backup_exported: 'Library exported to file successfully',
        toast_backup_imported: 'Library imported successfully!'
    },

    es: {
        code: 'ESP',
        flag: '🇪🇸',
        name: 'Español',

        nav_home: 'Inicio',
        nav_search: 'Buscar',
        nav_liked: 'Favoritos',
        nav_history: 'Historial',
        nav_profile: 'Perfil',
        nav_playlists_title: 'LISTAS DE REPRODUCCIÓN',
        btn_create_playlist: 'Crear lista',

        search_placeholder: 'Buscar música, artistas o mezclas...',
        source_all: 'Todos',
        source_sc: 'SoundCloud',
        source_yt: 'YouTube',

        genre_lofi: '☕ Lo-Fi Chill',
        genre_synthwave: '🌆 Synthwave',
        genre_phonk: '🏎️ Phonk Drift',
        genre_deephouse: '🎧 Deep House',
        genre_rock: '🎸 Rock',
        genre_gaming: '🎮 Gaming Mix',
        genre_ambient: '🌿 Ambient',
        genre_ua: '🇺🇦 Música Ucraniana',

        hero_tag: 'NUEVO NIVEL DE SONIDO',
        hero_title: 'Música Sin Límites',
        hero_desc: 'Transmisión instantánea desde SoundCloud y YouTube. Cero lag, ultra liviano.',
        hero_btn_mix: 'Reproducir mix',
        hero_btn_search: 'Buscar pista',
        section_popular: 'Pistas Populares',

        liked_header_tag: 'Tu Colección',
        liked_header_title: 'Pistas Favoritas',
        liked_header_meta: 'pistas en colección',
        btn_play_all: 'Reproducir todo',
        btn_shuffle: 'Aleatorio',
        empty_liked_title: 'Aún no tienes pistas guardadas',
        empty_liked_desc: 'Haz clic en el corazón de cualquier pista para guardarla aquí.',

        history_header_tag: 'Reproducciones recientes',
        history_header_title: 'Historial de reproducción',
        btn_clear_history: 'Borrar historial',
        empty_history_title: 'El historial está vacío',
        empty_history_desc: 'Escucha música y tus canciones recientes aparecerán aquí.',

        search_results_for: 'Resultados de búsqueda para:',
        search_count_found: 'pistas encontradas',
        search_empty_title: 'No se encontró nada',
        search_empty_desc: 'Prueba otro término de búsqueda o cambia la fuente.',
        search_initial_title: 'Ingrese el título de la pista o artista para buscar',

        profile_edit_btn: 'Editar perfil',
        profile_banner_btn: 'Cambiar portada',
        profile_badge_tooltip: 'Haga clic para seleccionar rol',
        role_selector_title: 'Seleccione su rol:',
        profile_tab_all: 'Todos',
        profile_tab_popular: 'Pistas populares',
        profile_tab_liked: 'Favoritos',
        profile_tab_playlists: 'Listas',
        profile_tab_history: 'Historial',
        profile_tab_admin: 'Panel de Admin',
        profile_tab_friends: 'Amigos',

        // Friends
        friends_section_title: 'Mis amigos en SoundFlow',
        friends_section_subtitle: 'Escuchen música juntos, compartan pistas y conecten con nuevos amigos.',
        btn_add_friend: 'Agregar amigo',
        modal_add_friend_title: 'Agregar nuevo amigo',
        modal_add_friend_desc: 'Ingrese el nombre de usuario o etiqueta de SoundFlow:',
        friends_listening_to: 'Escuchando',
        btn_listen_together: 'Escuchar',
        sidebar_friends_header: 'AMIGOS',

        role_admin: 'Administrador',
        role_moderator: 'Moderador',
        role_artist: 'Artista',
        role_listener: 'Oyente',

        stat_followers: 'Seguidores',
        stat_following: 'Siguiendo',
        stat_tracks: 'Pistas',
        sidebar_likes_header: 'LIKES',
        sidebar_view_all: 'Ver todo',
        sidebar_following_header: 'FOLLOWING',
        sidebar_empty_likes: 'Aún no hay pistas favoritas',

        admin_hero_pill: '👑 PRIVILEGES LEVEL: OWNER / ADMIN',
        admin_hero_status: '● SISTEMA ACTIVO',
        admin_hero_title: 'Panel de Control SoundFlow',
        admin_hero_desc: 'Acceso total al procesador de audio DSP, base de datos y ajustes avanzados.',
        admin_card_hq_title: 'Audio Estudio HQ',
        admin_card_hq_desc: 'Transmisión sin comprimir a 320 kbps de máxima fidelidad.',
        admin_card_hq_active: 'ACTIVADO (320 KBPS)',
        admin_card_bass_title: 'Realce de Graves (DSP)',
        admin_card_bass_desc: 'Filtro low-shelf por hardware para graves potentes de estudio.',
        admin_card_role_title: 'Selector de Rol y Estado',
        admin_card_role_desc: 'Seleccione el rol activo para su cuenta:',
        admin_card_banner_title: 'Estilos y Temas de Portada',
        admin_card_banner_desc: 'Elija un estilo predeterminado o personalice la portada:',
        preset_beige: 'Beige Aesthetic',
        preset_neon: 'Neon Sunset',
        preset_cyber: 'Cyberpunk',
        preset_midnight: 'Midnight',
        admin_card_cache_title: 'Limpieza y Optimización',
        admin_card_cache_desc: 'Libere instantáneamente la memoria RAM, caché y archivos temporales.',
        btn_clean_cache: 'Limpiar Caché y RAM',
        admin_card_backup_title: 'Copia de Seguridad',
        admin_card_backup_desc: 'Exporte o importe toda su biblioteca y preferencias en JSON.',
        btn_export_lib: 'Exportar Biblioteca',
        btn_import_lib: 'Importar Biblioteca',

        player_track_placeholder: 'Selecciona una pista para reproducir',
        player_artist_placeholder: 'SoundFlow',
        queue_drawer_title: 'Cola de reproducción',
        queue_now_playing: 'Sonando ahora',
        queue_next_up: 'Siguiente en cola',
        queue_empty_now: 'Nada en reproducción',
        queue_empty_next: 'Fin de la cola',
        btn_autoplay: 'Reproducción automática de canciones similares',
        toast_autoplay_on: 'Reproducción automática activada: canciones similares continuas ♾️',
        toast_autoplay_off: 'Reproducción automática desactivada',
        toast_finding_similar: 'Buscando canciones similares... 🎵',
        queue_similar_badge: 'Temática similar',

        modal_create_pl_title: 'Crear nueva lista',
        modal_create_pl_placeholder: 'Título de la lista...',
        modal_btn_cancel: 'Cancelar',
        modal_btn_create: 'Crear',
        modal_picker_pl_title: 'Agregar a lista',

        btn_sign_in: 'Iniciar sesión',
        menu_view_profile: 'Mi perfil de SoundCloud',
        menu_account_settings: 'Ajustes de cuenta',
        menu_sign_out: 'Cerrar sesión',

        toast_signed_out: 'Has cerrado sesión',
        toast_welcome: 'Bienvenido',
        toast_liked: 'Agregado a favoritos ❤️',
        toast_unliked: 'Eliminado de favoritos',
        toast_history_cleared: 'Historial borrado',
        toast_cache_cleared: '¡Caché limpiada y memoria RAM optimizada!',
        toast_role_switched: 'Rol cambiado a',
        toast_theme_applied: '¡Tema de portada aplicado!',
        toast_backup_exported: 'Biblioteca exportada con éxito',
        toast_backup_imported: '¡Biblioteca importada con éxito!'
    }
};

class SoundFlowI18n {
    constructor() {
        this.currentLang = localStorage.getItem('soundflow_language') || 'uk';
        if (!SOUNDFLOW_TRANSLATIONS[this.currentLang]) {
            this.currentLang = 'uk';
        }
    }

    init() {
        this.setupSwitcherUI();
        this.applyLanguage(this.currentLang);
    }

    t(key, fallback = '') {
        const langPack = SOUNDFLOW_TRANSLATIONS[this.currentLang] || SOUNDFLOW_TRANSLATIONS['uk'];
        return langPack[key] || fallback || key;
    }

    setLanguage(lang) {
        if (!SOUNDFLOW_TRANSLATIONS[lang]) return;
        this.currentLang = lang;
        localStorage.setItem('soundflow_language', lang);

        this.applyLanguage(lang);

        // Sync with backend if available
        if (window.pywebview && window.pywebview.api && window.pywebview.api.update_settings) {
            window.pywebview.api.update_settings({ language: lang }).catch(() => {});
        }

        // Notify app to re-render active view
        if (window.soundFlowApp && typeof window.soundFlowApp.onLanguageChanged === 'function') {
            window.soundFlowApp.onLanguageChanged(lang);
        }
    }

    applyLanguage(lang) {
        const pack = SOUNDFLOW_TRANSLATIONS[lang] || SOUNDFLOW_TRANSLATIONS['uk'];

        // Update button current label & flag
        const flagEl = document.getElementById('lang-current-flag');
        const codeEl = document.getElementById('lang-current-code');
        if (flagEl) flagEl.textContent = pack.flag;
        if (codeEl) codeEl.textContent = pack.code;

        // Update active class in dropdown options
        document.querySelectorAll('.lang-option-btn').forEach(btn => {
            btn.classList.toggle('active', btn.dataset.lang === lang);
        });

        // Translate elements with data-i18n
        document.querySelectorAll('[data-i18n]').forEach(el => {
            const key = el.getAttribute('data-i18n');
            if (pack[key] !== undefined) {
                el.textContent = pack[key];
            }
        });

        // Translate placeholders with data-i18n-placeholder
        document.querySelectorAll('[data-i18n-placeholder]').forEach(el => {
            const key = el.getAttribute('data-i18n-placeholder');
            if (pack[key] !== undefined) {
                el.setAttribute('placeholder', pack[key]);
            }
        });

        // Translate titles/tooltips with data-i18n-title
        document.querySelectorAll('[data-i18n-title]').forEach(el => {
            const key = el.getAttribute('data-i18n-title');
            if (pack[key] !== undefined) {
                el.setAttribute('title', pack[key]);
            }
        });

        // Update HTML lang attribute
        document.documentElement.lang = lang;
    }

    setupSwitcherUI() {
        const btn = document.getElementById('lang-menu-btn');
        const dropdown = document.getElementById('lang-dropdown');

        if (btn && dropdown) {
            btn.addEventListener('click', (e) => {
                e.stopPropagation();
                // Close user dropdown if open
                const userDropdown = document.getElementById('user-dropdown');
                if (userDropdown) userDropdown.style.display = 'none';

                dropdown.style.display = dropdown.style.display === 'none' ? 'flex' : 'none';
            });

            document.addEventListener('click', () => {
                dropdown.style.display = 'none';
            });

            dropdown.querySelectorAll('.lang-option-btn').forEach(opt => {
                opt.addEventListener('click', (e) => {
                    e.stopPropagation();
                    const targetLang = opt.dataset.lang;
                    dropdown.style.display = 'none';
                    this.setLanguage(targetLang);
                });
            });
        }
    }
}

// Global i18n instance
window.soundFlowI18n = new SoundFlowI18n();
