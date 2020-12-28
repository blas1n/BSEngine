#pragma once

#include <functional>

template <class R, class... Args>
class Delegate final
{
public:
	Delegate() noexcept = default;
	Delegate(std::nullptr_t) noexcept : Delegate() {}

	template <class Fn>
	Delegate(Fn&& fn)
		: func(fn) {}

	template <class T>
	Delegate(T&& obj, R(T::*fn)(Args...))
		: func([obj, fn](Args&&... args) { return obj.*fn(std::forward<Args>(args)...) }) {}

	Delegate(const Delegate&) = default;
	Delegate(Delegate&&) noexcept = default;

	Delegate& operator=(const Delegate&) = default;
	Delegate& operator=(Delegate&&) noexcept = default;

	~Delegate() = default;
	
	Delegate& operator=(std::nullptr_t)
	{
		func = nullptr;
		return *this;
	}

	template <class Fn>
	Delegate& operator=(Fn&& fn)
	{
		return *this = Delegate(fn);
	}

	template <class T>
	Delegate& operator=(T&& obj, R(T::*fn)(Args...))
	{
		return *this = Delegate(obj, fn);
	}

	[[nodiscard]] R operator()(Args&&... args)
	{
		return func(std::forward<Args>(args)...);
	}
	
	[[nodiscard]] operator bool() const noexcept
	{
		return static_cast<bool>(func);
	}

	[[nodiscard]] bool IsBound() const noexcept
	{
		return static_cast<bool>(func);
	}

private:
	std::function<R(Args...)> func;
};