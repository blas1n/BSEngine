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

	explicit InputAction(const Json& json);

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

	explicit InputAxis(const Json& json);

	InputCode code;
	float scale;
};

class INPUT_API InputManager : public Manager, private Accessor<class WindowManager>
{
public:
	[[nodiscard]] bool Init() noexcept override;
	[[nodiscard]] bool Update(float deltaTime) noexcept override;
	void Release() noexcept override;

	const IntVector2& GetMousePos() const noexcept { return mousePos; }

private:
	bool ReadKeyboard();
	bool ReadMouse();

private:
	struct InputImpl* impl;

	uint8 keyState[256];
	uint8 mouseState[8];
	IntVector2 mousePos;
};
