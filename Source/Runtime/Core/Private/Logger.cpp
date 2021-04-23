#include "Logger.h"
#include <filesystem>
#include "spdlog/spdlog.h"
#include "spdlog/sinks/basic_file_sink.h"
#include "spdlog/sinks/stdout_color_sinks.h"

namespace
{
    [[nodiscard]] constexpr spdlog::level::level_enum ToSpdLogLevel(LogVerbosity verbosity) noexcept
    {
        constexpr spdlog::level::level_enum mapper[]{ spdlog::level::debug, spdlog::level::info,
            spdlog::level::info, spdlog::level::warn, spdlog::level::err, spdlog::level::critical };

        return mapper[static_cast<BSBase::uint8>(verbosity)];
    }
}

Logger::Logger(StringView name, StringView dir)
{
    using namespace std::filesystem;

    if (const auto logger = spdlog::get(CastCharSet<char>(name)))
    {
        impl = logger;
        return;
    }

    auto path = current_path().parent_path() / "Saved" / "Logs";
    create_directories(dir);
    
    path /= fmt::format(STR("{}.log"), dir);
    impl = spdlog::basic_logger_mt(CastCharSet<char>(name), path.string());
    impl->set_level(spdlog::level::debug);
    impl->flush_on(spdlog::level::critical);
}

Logger::Logger(StringView name, const Console&)
{
    using namespace std::filesystem;

    if (const auto logger = spdlog::get(CastCharSet<char>(name)))
    {
        impl = logger;
        return;
    }

    impl = spdlog::stdout_color_mt("console");
    impl->flush_on(spdlog::level::critical);
}

Logger::~Logger()
{
    impl->flush();
}

void Logger::Log(LogVerbosity verbosity, const String& message)
{
    impl->log(ToSpdLogLevel(verbosity), CastCharSet<char>(StringView{ message }));
}
