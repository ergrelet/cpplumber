#include <cstdio>
#include <memory>
#include <cwchar>

#include "header.h"

#define PREPROCESSOR_STRING_LITERAL "preprocessor_string_literal"
#define PREPROCESSOR_WIDE_STRING_LITERAL L"preprocessor_string_literal"

static const char* my_c_string = "c_string";
static const char* my_utf8_string = u8"utf8_string";
static const wchar_t* my_wide_string = L"wide_string";
static const char16_t* my_utf16_string = u"utf16_string";
static const char32_t* my_utf32_string = U"utf32_string";
// TODO: user-defined literals

static const char* my_raw_string = R"(raw_string)";
static const char* my_raw_utf8_string = u8R"(raw_utf8_string)";
static const wchar_t* my_wide_raw_string = LR"(wide_raw_string)";
static const char16_t* my_raw_utf16_string = uR"(raw_utf16_string)";
static const char32_t* my_raw_utf32_string = UR"(raw_utf32_string)";

#ifdef DEF_TEST
static const char* def_test_string = "def_test";
#endif
static const char* my_concatenated_string = "concatenated" "_string";
static const char* my_multiline_string = R"(multiline
string)";
static const char* my_escaped_string = "\'\"\n\t\a\b|\x90|\220|\u9999|\U00009999|😂";
// static const char* my_commented_string = "commented_string";

struct MyStruct {};
struct MyClass {};

int main() {
    printf("%s\n", PREPROCESSOR_STRING_LITERAL);
    wprintf(L"%s\n", PREPROCESSOR_WIDE_STRING_LITERAL);
    printf("%s\n", included_string_literal);

    // Force the generation of some RTTI
    std::shared_ptr<MyStruct> struct_ptr = std::make_unique<MyStruct>();
    std::shared_ptr<MyClass> class_ptr = std::make_unique<MyClass>();

    return 0;
}
