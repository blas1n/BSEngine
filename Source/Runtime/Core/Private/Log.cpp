#include "Log.h"
#include <filesystem>
#include <spdlog/spdlog.h>
#include <spdlog/sinks/daily_file_sink.h>
#include <spdlog/sinks/stdout_color_sinks.h>

namespace
{
    [[nodiscard]] constexpr spdlog::level::level_enum ToSpdLogLevel(LogVerbosity verbosity) noexcept
    {
        constexpr spdlog::level::level_enum mapper[]{ spdlog::level::debug, spdlog::level::info,
            spdlog::level::info, spdlog::level::warn, spdlog::level::err, spdlog::level::critical };

        return mapper[static_cast<uint8>(verbosity)];
    }
}

class Logger final
{
public:
    [[nodiscard]] static Logger& Get()
    {
        static Logger logger;
        return logger;
    }

    Logger()
    {
        using namespace std::filesystem;
        console = spdlog::stdout_color_mt("console");

        const auto path = current_path();
        auto dir = path.parent_path() / "Saved" / "Logs";
        create_directories(dir);
        dir /= fmt::format("{}.log", path.filename().string());

        file = spdlog::daily_logger_mt("file", dir.string());
        file->set_level(spdlog::level::debug);
        file->flush_on(spdlog::level::critical);
        console->flush_on(spdlog::level::critical);
    }

    ~Logger()
    {
        console->flush();
        file->flush();
    }

    void Log(const LogCategory& category, LogVerbosity verbosity, std::string message)
    {
        const auto msg = fmt::format("{}: {}", category.name, message);
        file->log(ToSpdLogLevel(verbosity), msg);

#ifdef NDEBUG
        if (verbosity != LogVerbosity::Log)
#endif
            console->log(ToSpdLogLevel(verbosity), msg);
    }

private:
    std::shared_ptr<spdlog::logger> console;
    std::shared_ptr<spdlog::logger> file;
};

void Log(const LogCategory& category, LogVerbosity level, std::string message)
{
    Logger::Get().Log(category, level, message);
}