#pragma once

#include <string>
#include <unordered_map>

class CORE_API ConfigFile final
{
    class ConfigSection final
    {
    public:
        ConfigSection(const std::unordered_map<std::string, std::string>& inData)
            : data(inData) {}

        ConfigSection(std::unordered_map<std::string, std::string>&& inData)
            : data(std::move(inData)) {}
        
        [[nodiscard]] const std::string* operator[](const std::string& key) const noexcept;

    private:
        std::unordered_map<std::string, std::string> data;
    };

public:
    ConfigFile() = default;

    ConfigFile(const std::string& fileName)
        : ConfigFile()
    {
        LoadFromFile(fileName);
    }

    bool LoadFromFile(const std::string& fileName) noexcept;
    void Clear() noexcept;

    [[nodiscard]] bool IsAvailable() const noexcept { return isAvailable; }
    [[nodiscard]] operator bool() const noexcept { return isAvailable; }

    [[nodiscard]] bool IsExistSection(const std::string& section) const noexcept
    {
        return data.find(section) != data.cend();
    }

    [[nodiscard]] const std::string* operator()(const std::string& sectionName, const std::string& keyName) const noexcept;

private:
    std::unordered_map<std::string, ConfigSection> data;
    bool isAvailable;
};