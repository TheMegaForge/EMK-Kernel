#include <Windows.h>
#include <consoleapi2.h>
#include <processenv.h>
#include <processthreadsapi.h>
#include <winbase.h>

void mainCRTStartup() {
    COORD WriteCoord = {
        .X = 0,
        .Y = 0
    };
    WriteConsoleOutputCharacterA(GetStdHandle(STD_OUTPUT_HANDLE), "Hallo Welt\n", 11, WriteCoord, NULL);
    ExitProcess(0);
}
