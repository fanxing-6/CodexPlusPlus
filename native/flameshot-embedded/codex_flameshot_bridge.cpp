#include "codex_flameshot_bridge.h"

#include "core/capturerequest.h"
#include "core/flameshot.h"

#include <QApplication>
#include <QCoreApplication>
#include <QEventLoop>
#include <QFileInfo>
#include <QNetworkProxyFactory>
#include <QString>

#include <cstdlib>
#include <cstring>
#include <exception>
#include <mutex>

namespace {

std::mutex g_capture_mutex;

void set_message(CodexFlameshotCaptureResult* result, const char* message)
{
    if (result == nullptr) {
        return;
    }
    if (result->message != nullptr) {
        std::free(result->message);
        result->message = nullptr;
    }
    if (message == nullptr) {
        return;
    }
    const size_t len = std::strlen(message);
    auto* copy = static_cast<char*>(std::malloc(len + 1));
    if (copy == nullptr) {
        return;
    }
    std::memcpy(copy, message, len + 1);
    result->message = copy;
}

bool ensure_qapplication(CodexFlameshotCaptureResult* result)
{
    static int argc = 1;
    static char app_name[] = "Codex++";
    static char* argv[] = { app_name, nullptr };
    static QApplication* owned_app = nullptr;

    auto* current = QCoreApplication::instance();
    if (current != nullptr) {
        if (qobject_cast<QApplication*>(current) == nullptr) {
            set_message(result, "当前进程已有非 GUI Qt application，无法嵌入 Flameshot");
            return false;
        }
        return true;
    }

    owned_app = new QApplication(argc, argv);
    QCoreApplication::setApplicationName(QStringLiteral("Codex++"));
    QCoreApplication::setOrganizationName(QStringLiteral("Codex++"));
    QNetworkProxyFactory::setUseSystemConfiguration(true);
    return owned_app != nullptr;
}

int request_capture_and_wait(const CaptureRequest& request)
{
    auto* flameshot = Flameshot::instance();
    QEventLoop loop;
    int exit_code = CODEX_FLAMESHOT_UNAVAILABLE;

    QObject::connect(flameshot,
                     &Flameshot::captureTaken,
                     &loop,
                     [&]() {
                         exit_code = CODEX_FLAMESHOT_OK;
                         loop.quit();
                     },
                     Qt::SingleShotConnection);
    QObject::connect(flameshot,
                     &Flameshot::captureFailed,
                     &loop,
                     [&]() {
                         exit_code = CODEX_FLAMESHOT_CANCELLED;
                         loop.quit();
                     },
                     Qt::SingleShotConnection);

    flameshot->requestCapture(request);
    loop.exec();
    return exit_code;
}

} // namespace

int codex_flameshot_capture_region(const CodexFlameshotCaptureRequest* request,
                                   CodexFlameshotCaptureResult* result)
{
    if (result != nullptr) {
        result->message = nullptr;
    }
    if (request == nullptr || request->output_path == nullptr ||
        request->output_path[0] == '\0') {
        set_message(result, "缺少截图输出路径");
        return CODEX_FLAMESHOT_UNAVAILABLE;
    }

    std::lock_guard<std::mutex> lock(g_capture_mutex);
    try {
        if (!ensure_qapplication(result)) {
            return CODEX_FLAMESHOT_UNAVAILABLE;
        }

        Flameshot::setOrigin(Flameshot::CLI);
        const QString output_path = QString::fromUtf8(request->output_path);
        CaptureRequest capture(CaptureRequest::GRAPHICAL_MODE,
                               request->delay_ms,
                               output_path);
        capture.addSaveTask(output_path);
        if (request->accept_on_select != 0) {
            capture.addTask(CaptureRequest::ACCEPT_ON_SELECT);
        }

        const int code = request_capture_and_wait(capture);
        if (code == CODEX_FLAMESHOT_OK && QFileInfo::exists(output_path)) {
            return CODEX_FLAMESHOT_OK;
        }
        if (code == CODEX_FLAMESHOT_OK) {
            set_message(result, "已取消区域截图或未保存截图");
            return CODEX_FLAMESHOT_CANCELLED;
        }
        if (code == CODEX_FLAMESHOT_CANCELLED) {
            set_message(result, "已取消区域截图");
            return CODEX_FLAMESHOT_CANCELLED;
        }
        set_message(result, "Flameshot 嵌入式截图未返回有效结果");
        return CODEX_FLAMESHOT_UNAVAILABLE;
    } catch (const std::exception& error) {
        set_message(result, error.what());
        return CODEX_FLAMESHOT_UNAVAILABLE;
    } catch (...) {
        set_message(result, "Flameshot 嵌入式截图发生未知异常");
        return CODEX_FLAMESHOT_UNAVAILABLE;
    }
}

void codex_flameshot_capture_result_free(CodexFlameshotCaptureResult* result)
{
    if (result == nullptr) {
        return;
    }
    if (result->message != nullptr) {
        std::free(result->message);
        result->message = nullptr;
    }
}
