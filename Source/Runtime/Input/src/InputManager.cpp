#include "InputManager.h"
#include <dinput.h>
#include "BSMath.h"
#include "WindowManager.h"

#pragma comment(lib, "dinput8.lib")
#pragma comment(lib, "dxguid.lib")

struct InputImpl final
{
	IDirectInput8* directInput;
	IDirectInputDevice8* keyboard;
	IDirectInputDevice8* mouse;
	DIMOUSESTATE2 mouseState;
};

namespace
{
	template <class... Ts>
	struct Overload : Ts...
	{
		using Ts::operator()...;
	};

	template <class... Ts>
	Overload(Ts ...)->Overload<Ts...>;

	String ToString(InputCode code)
	{
		const auto type = std::visit(
			Overload{
				[] (KeyCode) { return ReservedName::KeyCode; },
				[] (MouseCode) { return ReservedName::MouseCode; },
				[] (MouseAxis) { return ReservedName::MouseAxis; }
			}, code);

		Name str = std::visit(
			Overload{
				[] (KeyCode code) { return FromKeyCode(code); },
				[] (MouseCode code) { return FromMouseCode(code); },
				[] (MouseAxis axis) { return FromMouseAxis(axis); }
			}, code);

		return Name{ type }.ToString() + str.ToString();
	}

	std::optional<InputCode> FromString(String str)
	{
		const static std::unordered_map<Name, Delegate<std::optional<InputCode>(Name)>, Hash<Name>> parser
		{
			std::make_pair(ReservedName::KeyCode, ToKeyCode),
			std::make_pair(ReservedName::MouseCode, ToMouseCode),
			std::make_pair(ReservedName::MouseAxis, ToMouseAxis)
		};

		const auto del = str.find(STR('.'));
		if (del == String::npos)
			return std::nullopt;

		const auto type = str.substr(0, del);
		const auto code = str.substr(del + 1);

		auto iter = parser.find(Name{ type.c_str() });
		if (iter == parser.cend())
			return std::nullopt;

		return iter->second(Name{ code.c_str() });
	}

	void DeserializeCode(const Json& json, InputCode& code)
	{
		const auto str = json["code"].get<std::string>();
		const auto codeValue = FromString(CastCharSet<Char>(std::string_view{ str.c_str() }));
		if (codeValue)
			code = codeValue.value();
	}
}

Json InputAction::Serialize() const
{
	Json json = Json::object();
	json["code"] = CastCharSet<char>(StringView{ ToString(code).c_str() });
	Json modes = json["mode"] = Json::array();
	
	for (const auto mod : FromKeyMode(mode))
		modes.emplace_back(CastCharSet<char>(StringView{ mod.ToString().c_str() }));

	return json;
}

void InputAction::Deserialize(const Json& json)
{
	DeserializeCode(json, code);
	mode = KeyMode::None;

	for (const auto mod : json["mode"])
	{
		const auto modeAnsiStr = mod.get<std::string>();
		const auto modeStr = CastCharSet<Char>(std::string_view{ modeAnsiStr.c_str() });
		mode |= ToKeyMode(Name{ modeStr.c_str() }).value();
	}
}

Json InputAxis::Serialize() const
{
	Json json = Json::object();
	json["code"] = CastCharSet<char>(StringView{ ToString(code).c_str() });
	json["scale"] = scale;
	return json;
}

void InputAxis::Deserialize(const Json& json)
{
	DeserializeCode(json, code);
	scale = json["scale"].get<float>();
}

Json AxisConfig::Serialize() const
{
	Json json = Json::object();
	json["deadZone"] = deadZone;
	json["sensitivity"] = sensitivity;
	return json;
}

void AxisConfig::Deserialize(const Json& json)
{
	deadZone = json["deadZone"].get<float>();
	sensitivity = json["sensitivity"].get<float>();
}

bool InputManager::Init() noexcept
{
	impl = new InputImpl;

	const auto hInst = reinterpret_cast<HINSTANCE>
		(Accessor<WindowManager>::GetManager()->GetHandle().hInstance);

	HRESULT result = DirectInput8Create(hInst, DIRECTINPUT_VERSION,
		IID_IDirectInput8, reinterpret_cast<void**>(&impl->directInput), nullptr);

	if (FAILED(result)) return false;

	result = impl->directInput->CreateDevice(GUID_SysKeyboard, &impl->keyboard, nullptr);
	if (FAILED(result)) return false;
	
	result = impl->keyboard->SetDataFormat(&c_dfDIKeyboard);
	if (FAILED(result)) return false;

	const auto hWnd = reinterpret_cast<HWND>
		(Accessor<WindowManager>::GetManager()->GetHandle().hInstance);

	result = impl->keyboard->SetCooperativeLevel(hWnd, DISCL_FOREGROUND | DISCL_EXCLUSIVE);
	if (FAILED(result)) return false;

	result = impl->keyboard->Acquire();
	if (FAILED(result)) return false;

	result = impl->directInput->CreateDevice(GUID_SysMouse, &impl->mouse, nullptr);
	if (FAILED(result)) return false;

	result = impl->mouse->SetDataFormat(&c_dfDIMouse2);
	if (FAILED(result)) return false;

	result = impl->mouse->SetCooperativeLevel(hWnd, DISCL_FOREGROUND | DISCL_NONEXCLUSIVE);
	if (FAILED(result)) return false;

	result = impl->mouse->Acquire();
	if (FAILED(result)) return false;
	
	return true;
}

bool InputManager::Update(float deltaTime) noexcept
{
	bool result = ReadKeyboard();
	if(!result) return false;
	
	result = ReadMouse();
	if(!result) return false;
	
	return true;
}

void InputManager::Release() noexcept
{
	if (impl->mouse)
	{
		impl->mouse->Unacquire();
		impl->mouse->Release();
		impl->mouse = nullptr;
	}

	if (impl->keyboard)
	{
		impl->keyboard->Unacquire();
		impl->keyboard->Release();
		impl->keyboard = nullptr;
	}

	if (impl->directInput)
	{
		impl->directInput->Release();
		impl->directInput = nullptr;
	}

	delete impl;
}

float InputManager::GetAxisValue(Name name) const noexcept
{
	const auto iter = axises.find(name);
	if (iter != axises.cend()) return 0.0f;

	float value = 0.0f;

	for (auto axis : iter->second)
		value += GetAxisValue(axis);

	return value;
}

float InputManager::GetAxisValue(InputAxis axis) const noexcept
{
	const auto get = Overload{
			[this] (KeyCode code) {	return GetValue(code) ? 1.0f : 0.0f; },
			[this] (MouseCode code) { return GetValue(code) ? 1.0f : 0.0f; },
			[this] (MouseAxis axis)	{ return GetAxisValue(axis); }
	};

	return std::visit(get, axis.code) * axis.scale;
}

float InputManager::GetAxisValue(MouseAxis axis) const noexcept
{
	if (static_cast<uint8>(axis) > static_cast<uint8>(MouseAxis::Wheel))
		return 0.0f;

	return FilterValue(axis, static_cast<float>(mouseAxis[static_cast<uint8>(axis)]));
}

bool InputManager::GetValue(Name name) const noexcept
{
	const auto iter = actions.find(name);
	if (iter != actions.cend()) return false;

	for (auto action : iter->second)
		if (GetValue(action))
			return true;

	return false;
}

bool InputManager::GetValue(InputAction action) const noexcept
{
	const auto get = Overload{
			[this](KeyCode code) { return GetValue(code); },
			[this](MouseCode code) { return GetValue(code); },
			[this](MouseAxis axis) { return GetAxisValue(axis) != 0.0f; }
	};

	return std::visit(get, action.code) && GetModeValue(action.mode);
}

bool InputManager::GetValue(KeyCode code) const noexcept
{
	if (static_cast<uint8>(code) > static_cast<uint8>(KeyCode::Sleep))
		return false;

	return keyState[static_cast<uint8>(code)] & 0x80;
}

bool InputManager::GetValue(MouseCode code) const noexcept
{
	if (static_cast<uint8>(code) > static_cast<uint8>(MouseCode::X5))
		return false;

	return mouseState[static_cast<uint8>(code)] & 0x80;
}

bool InputManager::ReadKeyboard() noexcept
{
	const HRESULT result = impl->keyboard->GetDeviceState(
		sizeof(keyState), reinterpret_cast<LPVOID>(&keyState));
	
	if (FAILED(result))
	{
		if ((result == DIERR_INPUTLOST) || (result == DIERR_NOTACQUIRED))
			impl->keyboard->Acquire();
		else
			return false;
	}

	return true;
}

bool InputManager::ReadMouse() noexcept
{
	const HRESULT result = impl->mouse->GetDeviceState(
		sizeof(DIMOUSESTATE2), reinterpret_cast<LPVOID>(&impl->mouseState));
	
	if (FAILED(result))
	{
		if ((result == DIERR_INPUTLOST) || (result == DIERR_NOTACQUIRED))
			impl->mouse->Acquire();
		else
			return false;
	}

	mouseAxis = IntVector3{ impl->mouseState.lX, impl->mouseState.lY, -impl->mouseState.lZ };
	memcpy(mouseState, impl->mouseState.rgbButtons, 8);
	return true;
}

float InputManager::FilterValue(InputCode code, float value) const noexcept
{
	const auto config = axisConfigs.find(code);
	
	if (config != axisConfigs.cend())
	{
		const auto cfg = config->second;
		
		value = value >= 0.0f
			? Max(0, Lerp(0.0f, 1.0f, GetRangePct(value, cfg.deadZone, 1.0f)))
			: Min(0, Lerp(-1.0f, 0.0f, GetRangePct(value, -1.0f, cfg.deadZone)));

		value *= cfg.sensitivity;
	}

	return value;
}

bool InputManager::GetModeValue(KeyMode mode) const noexcept
{
	bool ret = true;

	for (uint8 i = 1; i <= ModeNum; ++i)
		if (static_cast<uint8>(mode) & 1 << i)
			ret = ret && GetSimpleModeValue(i - 1);

	return ret;
}

bool InputManager::GetSimpleModeValue(uint8 mode) const noexcept
{
	const static std::vector<KeyCode> mapper[3]
	{
		{ KeyCode::LShift, KeyCode::RShift },
		{ KeyCode::LControl, KeyCode::RControl },
		{ KeyCode::LMenu, KeyCode::RMenu }
	};

	for (const auto code : mapper[mode])
		if (GetValue(code))
			return true;

	return false;
}
