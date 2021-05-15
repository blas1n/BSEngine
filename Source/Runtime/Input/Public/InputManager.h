#pragma once

#include "Core.h"
#include "Accessor.h"
#include "Manager.h"
#include <variant>
#include "InputCode.h"

using InputCode = std::variant<KeyCode, MouseCode, MouseAxis>;

struct INPUT_API InputAction final
{
	constexpr InputAction(KeyCode inCode, KeyMode inMode = KeyMode::None) noexcept
		: code(inCode), mode(inMode) {}

	constexpr InputAction(MouseCode inCode, KeyMode inMode = KeyMode::None) noexcept
		: code(inCode), mode(inMode) {}

	constexpr InputAction(MouseAxis inAxis, KeyMode inMode = KeyMode::None) noexcept
		: code(inAxis), mode(inMode) {}


	Json Serialize() const;
	void Deserialize(const Json& json);

	InputCode code;
	KeyMode mode;
};

struct INPUT_API InputAxis final
{
	constexpr InputAxis(KeyCode inCode, float inScale = 1.0f) noexcept
		: code(inCode), scale(inScale) {}
	
	constexpr InputAxis(MouseCode inCode, float inScale = 1.0f) noexcept
		: code(inCode), scale(inScale) {}
	
	constexpr InputAxis(MouseAxis inAxis, float inScale = 1.0f) noexcept
		: code(inAxis), scale(inScale) {}

	Json Serialize() const;
	void Deserialize(const Json& json);

	InputCode code;
	float scale;
};

struct AxisConfig final
{
	float deadZone = 0.0f;
	float sensitivity = 1.0f;

	Json Serialize() const;
	void Deserialize(const Json& json);
};

class INPUT_API InputManager : public Manager, private Accessor<class WindowManager>
{
public:
	[[nodiscard]] bool Init() noexcept override;
	[[nodiscard]] bool Update(float deltaTime) noexcept override;
	void Release() noexcept override;

	[[nodiscard]] float GetAxisValue(Name name) const noexcept;
	[[nodiscard]] float GetAxisValue(InputAxis axis) const noexcept;
	[[nodiscard]] float GetAxisValue(MouseAxis axis) const noexcept;

	[[nodiscard]] bool GetValue(Name name) const noexcept;
	[[nodiscard]] bool GetValue(InputAction action) const noexcept;

	[[nodiscard]] bool GetValue(KeyCode code) const noexcept;
	[[nodiscard]] bool GetValue(MouseCode code) const noexcept;

private:
	[[nodiscard]] bool ReadKeyboard() noexcept;
	[[nodiscard]] bool ReadMouse() noexcept;

	[[nodiscard]] float FilterValue(InputCode code, float value) const noexcept;
	[[nodiscard]] bool GetModeValue(KeyMode mode) const noexcept;
	[[nodiscard]] bool GetSimpleModeValue(uint8 mode) const noexcept;

private:
	struct InputImpl* impl;

	std::unordered_map<Name, std::vector<InputAxis>, Hash<Name>> axises;
	std::unordered_map<Name, std::vector<InputAction>, Hash<Name>> actions;
	std::unordered_map<InputCode, AxisConfig> axisConfigs;

	uint8 keyState[256];
	uint8 mouseState[8];
	IntVector3 mouseAxis;
};
