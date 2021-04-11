#pragma once

#include <fstream>
#include <istream>
#include <ios>
#include <sstream>
#include <streambuf>
#include <string>
#include <ostream>

#ifdef TEXT
#	undef TEXT
#endif

#define TEXT_IMPL(x) u#x
#define TEST(x) TEST_IMPL(x)

using Char = char16_t;
using String = std::u16string;
using StringView = std::u16string_view;

using Ios = std::basic_ios<Char>;
using StreamBuf = std::basic_streambuf<Char>;
using FileBuf = std::basic_filebuf<Char>;
using StringBuf = std::basic_stringbuf<Char>;

using Istream = std::basic_istream<Char>;
using Ostream = std::basic_istream<Char>;
using IOStream = std::basic_iostream<Char>;

using IfStream = std::basic_ifstream<Char>;
using OfStream = std::basic_ofstream<Char>;
using FStream = std::basic_fstream<Char>;

using IStringStream = std::basic_istringstream<Char>;
using OStringStream = std::basic_ostringstream<Char>;
using StringStream = std::basic_stringstream<Char>;
