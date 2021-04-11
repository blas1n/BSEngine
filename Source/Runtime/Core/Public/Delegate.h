#pragma once

#include <utility>

// This code was created with reference to https://www.codeproject.com/Articles/1170503/The-Impossibly-Fast-Cplusplus-Delegates-Fixed
template <class R, class... Args>
class Delegate final
{
public:
	Delegate() noexcept
		: object(nullptr), stub(nullptr) {}

	Delegate(std::nullptr_t) noexcept : Delegate() {}

	Delegate(const Delegate& other) noexcept
		: object(other.object), stub(other.stub) {}

	Delegate(Delegate&& other) noexcept
		: object(std::move(other.object)), stub(std::move(other.stub)) {}

	Delegate& operator=(const Delegate& other) noexcept
	{
		object = other.object;
		stub = other.stub;
		return *this;
	}

	Delegate& operator=(Delegate&& other)
	{
		object = std::move(other.object);
		stub = std::move(other.stub);
		return *this;
	}

	~Delegate() = default;

	template <R(*Function)(Args...)>
	Delegate(R(*fn)(Args...)) : object(nullptr), stub(&FunctionStub<Function>) {}

	template <class T, R(*Method)(Args...)>
	Delegate(T* obj) : object(obj), stub(&MethodStub<Method>) {}

	template <class T, R(*Method)(Args...)>
	Delegate(const T* obj) : object(const_cast<T*>(obj)), stub(&ConstMethodStub<Method>) {}

	template <class Functor>
	Delegate(Functor&& fn) : object(&fn), stub(&FunctorStub<Functor>) {}

	R operator()(Args&&... args) const
	{
		if (!stub) return R();

		return (*stub)(object, std::forward<Args>(args)...);
	}

	void Clear() noexcept { object = stub = nullptr; }

	[[nodiscard]] bool IsBound() const noexcept { return stub; }
	[[nodiscard]] operator bool() const noexcept { return stub; }

private:
	template <R(*Function)(Args...)>
	static R FunctionStub(void* object, Args... args)
	{
		return (Function)(std::forward<Args>(args)...);
	}

	template <class T, R(T::*Method)(Args...)>
	static R MethodStub(void* object, Args... args)
	{
		T* ptr = static_cast<T*>(object);
		return (ptr->*Method)(std::forward<Args>(args)...);
	}
	
	template <class T, R(T::*Method)(Args...) const>
	static R ConstMethodStub(const void* object, Args... args)
	{
		const T* ptr = static_cast<const T*>(object);
		return (ptr->*Method)(std::forward<Args>(args)...);
	}
	
	template <class Functor>
	static R FunctorStub(void* object, Args... args)
	{
		Functor* ptr = static_cast<Functor*>(object);
		return (ptr->operator())(std::forward<Args>(args)...);
	}

private:
	using Stub = R(*)(void*, Args...);

	void* object;
	Stub stub;
};
