#pragma once

#include <cstring>
#include <type_traits>
#include <utility>

namespace Impl
{
	template <class R, class... Args>
	class DelegateInstBase
	{
	public:
		virtual ~DelegateInstBase() = default;

		virtual R Execute(const Args&... args) const = 0;
	};

	template <class R, class... Args>
	class DelegateInstFunction final : public DelegateInstBase<R, Args...>
	{
		using Func = R(*)(Args...);

	public:
		DelegateInstFunction(Func inFn)
			: fn(inFn) {}

		R Execute(const Args&... args) const override
		{
			return (*fn)(args...);
		}

	private:
		Func fn;
	};

	template <class Class, class R, class... Args>
	class DelegateInstMethod final : public DelegateInstBase<R, Args...>
	{
		using Func = R(Class::*)(Args...);

	public:
		DelegateInstMethod(Class* inInst, Func inFn)
			: inst(inInst), fn(inFn) {}

		R Execute(const Args&... args) const override
		{
			return (inst->*fn)(args...);
		}

	private:
		Class* inst;
		Func fn;
	};

	template <class Class, class R, class... Args>
	class DelegateInstConstMethod final : public DelegateInstBase<R, Args...>
	{
		static_assert(std::is_const_v<Class>);
		using Func = R(Class::*)(Args...) const;

	public:
		DelegateInstConstMethod(Class* inInst, Func inFn)
			: inst(inInst), fn(inFn) {}

		R Execute(const Args&... args) const override
		{
			return (inst->*fn)(args...);
		}

	private:
		Class* inst;
		Func fn;
	};

	template <class Functor, class R, class... Args>
	class DelegateInstFunctor final : public DelegateInstBase<R, Args...>
	{
		using Func = std::decay_t<Functor>;

	public:
		DelegateInstFunctor(const Func& inFn)
			: fn(inFn) {}

		DelegateInstFunctor(Func&& inFn)
			: fn(std::move(inFn)) {}

		R Execute(const Args&... args) const override
		{
			return fn(args...);
		}

	private:
		std::remove_const_t<Func> fn;
	};
}
