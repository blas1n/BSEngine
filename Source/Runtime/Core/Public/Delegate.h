#pragma once

#include <utility>
#include "DelegateInst.h"

template <class T>
class Delegate;

template <class R, class... Args>
class Delegate<R(Args...)> final
{
public:
	Delegate() noexcept : storage{ nullptr, nullptr } {}

	Delegate(std::nullptr_t) noexcept : Delegate() {}

	Delegate(const Delegate& other) noexcept : Delegate()
	{
		if (const auto* inst = other.GetInst())
			inst->CopyTo(storage);
	}

	Delegate(Delegate&& other) noexcept : Delegate()
	{
		if (const auto inst = other.GetInst())
			inst->MoveTo(storage);

		other.Clear();
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

		other.Clear();
		return *this;
	}

	Delegate(R(*fn)(Args...)) : Delegate()
	{
		Impl::DelegateInstFunction<R, Args...>::Create(storage, fn);
	}

	template <class T>
	Delegate(T* obj, R(T::* fn)(Args...)) : Delegate()
	{
		Impl::DelegateInstMethod<T, R, Args...>::Create(storage, obj, fn);
	}

	template <class T>
	Delegate(T* obj, R(T::* fn)(Args...) const) : Delegate()
	{
		Impl::DelegateInstConstMethod<T, R, Args...>::Create(storage, obj, fn);
	}

	template <class Func>
	Delegate(Func&& fn) : Delegate()
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

	[[nodiscard]] friend bool operator==(const Delegate& lhs, const Delegate& rhs)
	{
		const auto lhsInst = lhs.GetInst();
		const auto rhsInst = rhs.GetInst();

		if (!lhsInst) return !rhsInst;
		if (!rhsInst) return false;

		return lhsInst->EqualTo(*rhsInst);
	}

private:
	[[nodiscard]] Impl::DelegateInstBase<R, Args...>* GetInst() noexcept
	{
		return const_cast<Impl::DelegateInstBase<R, Args...>*>(static_cast<const Delegate*>(this)->GetInst());
	}

	[[nodiscard]] const Impl::DelegateInstBase<R, Args...>* GetInst() const noexcept
	{
		if (!storage[0]) return nullptr;

		if (const auto heap = GetHeap())
			return reinterpret_cast<const Impl::DelegateInstBase<R, Args...>*>(heap);
		
		return reinterpret_cast<const Impl::DelegateInstBase<R, Args...>*>(&storage);
	}

	[[nodiscard]] void* GetHeap() const noexcept
	{
		return (storage[0] && !storage[1]) ? storage[0] : nullptr;
	}

	[[nodiscard]] const void* GetHeap() noexcept
	{
		return (storage[0] && !storage[1]) ? storage[0] : nullptr;
	}

private:
	void* storage[2];
};

template <class T>
[[nodiscard]] bool operator!=(const Delegate<T>& lhs, const Delegate<T>& rhs)
{
	return !(lhs == rhs);
}
