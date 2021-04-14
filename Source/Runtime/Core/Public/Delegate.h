#pragma once

#include <utility>
#include "DelegateInst.h"

template <class R, class... Args>
class Delegate final
{
public:
	Delegate() noexcept : storage() {}

	Delegate(std::nullptr_t) noexcept : Delegate() {}

	Delegate(const Delegate& other) noexcept
	{
		if (const auto* inst = other.GetInst())
			inst->CopyTo(storage);
	}

	Delegate(Delegate&& other) noexcept
	{
		if (const auto inst = other.GetInst())
			inst->MoveTo(storage);
	}

	Delegate& operator=(const Delegate& other) noexcept
	{
		if (*this == other) return *this;

		Clear();

		if (const auto* inst = other.GetInst())
			inst->CopyTo(storage);

		return *this;
	}

	Delegate& operator=(Delegate&& other) noexcept
	{
		if (*this == other) return *this;

		Clear();
		
		if (const auto inst = other.GetInst())
			inst->MoveTo(storage);

		return *this;
	}

	Delegate(R(*fn)(Args...))
	{
		Impl::DelegateInstFunction<R, Args...>::Create(storage, fn);
	}

	template <class T>
	Delegate(T* obj, R(T::* fn)(Args...))
	{
		Impl::DelegateInstMethod<T, R, Args...>::Create(storage, obj, fn);
	}

	template <class T>
	Delegate(T* obj, R(T::* fn)(Args...) const)
	{
		Impl::DelegateInstConstMethod<T, R, Args...>::Create(storage, obj, fn);
	}

	template <class Func>
	Delegate(Func&& fn)
	{
		Impl::DelegateInstFunctor<Func, R, Args...>::Create(storage, std::forward<Func>(fn));
	}

	~Delegate() { Clear(); }

	R operator()(const Args&... args)
	{
		return IsBound() ? GetInst()->Execute(args...) : R();
	}

	R operator()(Args&&... args)
	{
		return IsBound() ? GetInst()->Execute(std::move(args)...) : R();
	}

	void Clear() noexcept
	{
		if (const auto inst = GetInst())
			inst->Clear();

		memset(storage, 0, sizeof(storage));
	}

	[[nodiscard]] bool IsBound() const noexcept { return storage[0]; }
	[[nodiscard]] operator bool() const noexcept { return IsBound(); }

private:
	[[nodiscard]] decltype(auto) GetInst() noexcept
	{
		if (const auto heap = GetHeap())
			return reinterpret_cast<Impl::DelegateInstBase<R, Args...>*>(heap);
		
		return reinterpret_cast<Impl::DelegateInstBase<R, Args...>*>(&storage);
	}

	[[nodiscard]] void* GetHeap() noexcept
	{
		return (storage[0] && !storage[1]) ? storage[0] : nullptr;
	}

private:
	void* storage[2];
};

template <class R, class... Args>
[[nodiscard]] bool operator==(const Delegate<R, Args...>& lhs, const Delegate<R, Args...>& rhs)
{
	return &lhs == &rhs;
}

template <class R, class... Args>
[[nodiscard]] bool operator!=(const Delegate<R, Args...>& lhs, const Delegate<R, Args...>& rhs)
{
	return !(lhs == rhs);
}
