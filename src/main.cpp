#include <gtk/gtk.h>
#include <webkit2/webkit2.h>

#include <algorithm>
#include <cctype>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <string>
#include <vector>

namespace fs = std::filesystem;

static const char* HOME_URL = "https://www.google.com";

struct Browser {
    GtkWidget* window{};
    GtkWidget* address{};
    GtkWidget* back{};
    GtkWidget* forward{};
    GtkWidget* reload{};
    GtkWidget* loading{};
    WebKitWebView* view{};
    fs::path profile;
    fs::path downloads;
};

static Browser app;

static fs::path data_dir() {
    const char* base = g_get_user_data_dir();
    return fs::path(base ? base : ".") / "SwiftLife";
}

static std::string trim(std::string value) {
    value.erase(value.begin(), std::find_if(value.begin(), value.end(), [](unsigned char c) { return !std::isspace(c); }));
    value.erase(std::find_if(value.rbegin(), value.rend(), [](unsigned char c) { return !std::isspace(c); }).base(), value.end());
    return value;
}

static std::string navigate_input(const std::string& raw) {
    const std::string value = trim(raw);
    if (value.empty()) return HOME_URL;
    if (value == "about:blank") return value;
    if (value.rfind("https://", 0) == 0 || value.rfind("http://", 0) == 0) return value;
    if (value.find(' ') != std::string::npos || value.find('.') == std::string::npos) {
        gchar* escaped = g_uri_escape_string(value.c_str(), nullptr, true);
        std::string result = "https://www.google.com/search?q=" + std::string(escaped ? escaped : "");
        g_free(escaped);
        return result;
    }
    return "https://" + value;
}

static void save_session(const char* uri) {
    if (!uri || g_str_has_prefix(uri, "about:")) return;
    std::error_code ec;
    fs::create_directories(app.profile, ec);
    std::ofstream out(app.profile / "last-session.url", std::ios::trunc);
    if (out) out << uri << '\n';
}

static std::string load_session() {
    std::ifstream in(app.profile / "last-session.url");
    std::string uri;
    std::getline(in, uri);
    uri = trim(uri);
    if (uri.rfind("https://", 0) == 0 || uri.rfind("http://", 0) == 0) return uri;
    return HOME_URL;
}

static void update_navigation(Browser* b) {
    gtk_widget_set_sensitive(b->back, webkit_web_view_can_go_back(b->view));
    gtk_widget_set_sensitive(b->forward, webkit_web_view_can_go_forward(b->view));
}

static void navigate(Browser* b, const std::string& input) {
    const auto url = navigate_input(input);
    webkit_web_view_load_uri(b->view, url.c_str());
}

static void on_address_activate(GtkEntry* entry, gpointer data) {
    auto* b = static_cast<Browser*>(data);
    navigate(b, gtk_entry_get_text(entry));
    gtk_widget_grab_focus(GTK_WIDGET(b->view));
}

static void on_back(GtkButton*, gpointer data) { webkit_web_view_go_back(static_cast<Browser*>(data)->view); }
static void on_forward(GtkButton*, gpointer data) { webkit_web_view_go_forward(static_cast<Browser*>(data)->view); }
static void on_reload(GtkButton*, gpointer data) { webkit_web_view_reload(static_cast<Browser*>(data)->view); }
static void on_home(GtkButton*, gpointer data) { navigate(static_cast<Browser*>(data), HOME_URL); }

static void on_load_changed(WebKitWebView* view, WebKitLoadEvent event, gpointer data) {
    auto* b = static_cast<Browser*>(data);
    if (event == WEBKIT_LOAD_STARTED) gtk_widget_show(b->loading);
    if (event == WEBKIT_LOAD_FINISHED) gtk_widget_hide(b->loading);
    if (event == WEBKIT_LOAD_COMMITTED || event == WEBKIT_LOAD_FINISHED) {
        const char* uri = webkit_web_view_get_uri(view);
        if (uri) {
            gtk_entry_set_text(GTK_ENTRY(b->address), uri);
            save_session(uri);
        }
        update_navigation(b);
    }
}

static void on_title_changed(GObject* object, GParamSpec*, gpointer data) {
    auto* b = static_cast<Browser*>(data);
    const char* title = webkit_web_view_get_title(WEBKIT_WEB_VIEW(object));
    std::string text = title && *title ? title : "SwiftLife";
    std::string window_title = text + " — SwiftLife";
    gtk_window_set_title(GTK_WINDOW(b->window), window_title.c_str());
}

static gboolean on_decide_policy(WebKitWebView*, WebKitPolicyDecision* decision, WebKitPolicyDecisionType type, gpointer) {
    if (type != WEBKIT_POLICY_DECISION_TYPE_NAVIGATION_ACTION) return FALSE;
    auto* action = webkit_navigation_policy_decision_get_navigation_action(WEBKIT_NAVIGATION_POLICY_DECISION(decision));
    auto* request = webkit_navigation_action_get_request(action);
    const char* uri = webkit_uri_request_get_uri(request);
    if (!uri) return TRUE;
    if (g_str_has_prefix(uri, "http://") || g_str_has_prefix(uri, "https://") || g_str_has_prefix(uri, "about:")) return FALSE;
    webkit_policy_decision_ignore(decision);
    return TRUE;
}

static WebKitWebView* on_create(WebKitWebView*, WebKitNavigationAction*, gpointer data) {
    return static_cast<Browser*>(data)->view;
}

static void on_download_started(WebKitWebContext*, WebKitDownload* download, gpointer data) {
    auto* b = static_cast<Browser*>(data);
    std::error_code ec;
    fs::create_directories(b->downloads, ec);
    auto* request = webkit_download_get_request(download);
    const char* uri = request ? webkit_uri_request_get_uri(request) : nullptr;
    std::string filename = "download";
    if (uri) {
        const char* slash = strrchr(uri, '/');
        if (slash && *(slash + 1)) filename = slash + 1;
    }
    for (char& c : filename) if (std::iscntrl(static_cast<unsigned char>(c)) || c == '/' || c == '\\' || c == ':') c = '_';
    auto target = b->downloads / filename;
    const std::string destination = "file://" + target.string();
    webkit_download_set_destination(download, destination.c_str());
}

static GtkWidget* button(const char* label, GCallback callback, Browser* b) {
    GtkWidget* item = gtk_button_new_with_label(label);
    gtk_widget_set_name(item, "toolbar-button");
    g_signal_connect(item, "clicked", callback, b);
    return item;
}

static void apply_css() {
    const char* css = R"CSS(
window { background:#0d1016; }
#toolbar { background:#11151d; border-bottom:1px solid #252c37; padding:10px 12px; }
#brand { color:#f5f7fb; font-weight:800; font-size:15px; padding:0 12px 0 2px; }
#toolbar-button { background:#171c25; color:#cbd2dc; border:1px solid #2b3442; border-radius:9px; padding:6px 10px; min-width:34px; }
#toolbar-button:hover { background:#232a36; color:#ffffff; }
#address { background:#0d1219; color:#eef2f7; border:1px solid #303a49; border-radius:12px; padding:9px 13px; }
#address:focus { border-color:#7560dc; }
#loading { color:#8b76ff; padding:0 8px; }
)CSS";
    GtkCssProvider* provider = gtk_css_provider_new();
    gtk_css_provider_load_from_data(provider, css, -1, nullptr);
    gtk_style_context_add_provider_for_screen(gdk_screen_get_default(), GTK_STYLE_PROVIDER(provider), GTK_STYLE_PROVIDER_PRIORITY_APPLICATION);
    g_object_unref(provider);
}

static void activate(GtkApplication* application, gpointer) {
    std::error_code ec;
    app.profile = data_dir() / "profile";
    const char* download_dir = g_get_user_special_dir(G_USER_DIRECTORY_DOWNLOAD);
    app.downloads = fs::path(download_dir ? download_dir : data_dir().c_str()) / "SwiftLife";
    fs::create_directories(app.profile, ec);
    fs::create_directories(app.downloads, ec);

    app.window = gtk_application_window_new(application);
    gtk_window_set_default_size(GTK_WINDOW(app.window), 1440, 900);
    gtk_window_set_title(GTK_WINDOW(app.window), "SwiftLife");
    gtk_window_set_position(GTK_WINDOW(app.window), GTK_WIN_POS_CENTER);

    apply_css();
    GtkWidget* root = gtk_box_new(GTK_ORIENTATION_VERTICAL, 0);
    gtk_container_add(GTK_CONTAINER(app.window), root);

    GtkWidget* toolbar = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 7);
    gtk_widget_set_name(toolbar, "toolbar");
    gtk_box_pack_start(GTK_BOX(root), toolbar, FALSE, FALSE, 0);

    GtkWidget* brand = gtk_label_new("⚡ SwiftLife");
    gtk_widget_set_name(brand, "brand");
    gtk_box_pack_start(GTK_BOX(toolbar), brand, FALSE, FALSE, 0);
    app.back = button("‹", G_CALLBACK(on_back), &app);
    app.forward = button("›", G_CALLBACK(on_forward), &app);
    app.reload = button("↻", G_CALLBACK(on_reload), &app);
    gtk_box_pack_start(GTK_BOX(toolbar), app.back, FALSE, FALSE, 0);
    gtk_box_pack_start(GTK_BOX(toolbar), app.forward, FALSE, FALSE, 0);
    gtk_box_pack_start(GTK_BOX(toolbar), app.reload, FALSE, FALSE, 0);
    GtkWidget* home = button("⌂", G_CALLBACK(on_home), &app);
    gtk_box_pack_start(GTK_BOX(toolbar), home, FALSE, FALSE, 0);

    app.address = gtk_entry_new();
    gtk_widget_set_name(app.address, "address");
    gtk_entry_set_placeholder_text(GTK_ENTRY(app.address), "Adres veya web'de ara…");
    gtk_box_pack_start(GTK_BOX(toolbar), app.address, TRUE, TRUE, 4);
    g_signal_connect(app.address, "activate", G_CALLBACK(on_address_activate), &app);
    app.loading = gtk_label_new("●");
    gtk_widget_set_name(app.loading, "loading");
    gtk_widget_hide(app.loading);
    gtk_box_pack_end(GTK_BOX(toolbar), app.loading, FALSE, FALSE, 0);

    auto* manager = webkit_website_data_manager_new(
        "base-data-directory", app.profile.c_str(),
        "base-cache-directory", (app.profile / "cache").c_str(), nullptr);
    auto* context = webkit_web_context_new_with_website_data_manager(manager);
    webkit_web_context_set_cache_model(context, WEBKIT_CACHE_MODEL_WEB_BROWSER);
    g_signal_connect(context, "download-started", G_CALLBACK(on_download_started), &app);

    app.view = WEBKIT_WEB_VIEW(webkit_web_view_new_with_context(context));
    gtk_widget_set_hexpand(GTK_WIDGET(app.view), TRUE);
    gtk_widget_set_vexpand(GTK_WIDGET(app.view), TRUE);
    gtk_box_pack_end(GTK_BOX(root), GTK_WIDGET(app.view), TRUE, TRUE, 0);

    g_signal_connect(app.view, "load-changed", G_CALLBACK(on_load_changed), &app);
    g_signal_connect(app.view, "notify::title", G_CALLBACK(on_title_changed), &app);
    g_signal_connect(app.view, "decide-policy", G_CALLBACK(on_decide_policy), &app);
    g_signal_connect(app.view, "create", G_CALLBACK(on_create), &app);

    auto* settings = webkit_web_view_get_settings(app.view);
    webkit_settings_set_enable_developer_extras(settings, TRUE);
    webkit_settings_set_enable_smooth_scrolling(settings, TRUE);
    webkit_settings_set_enable_media_stream(settings, TRUE);

    gtk_widget_show_all(app.window);
    gtk_widget_hide(app.loading);
    navigate(&app, load_session());
    gtk_widget_grab_focus(GTK_WIDGET(app.view));
}

int main(int argc, char** argv) {
    GtkApplication* application = gtk_application_new("com.swiftlife.Browser", G_APPLICATION_DEFAULT_FLAGS);
    g_signal_connect(application, "activate", G_CALLBACK(activate), nullptr);
    const int status = g_application_run(G_APPLICATION(application), argc, argv);
    g_object_unref(application);
    return status;
}
