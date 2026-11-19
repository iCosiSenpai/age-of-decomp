// Adapted from iCosiSenpai/nx-to-win @752b214a047c21fec72253b03a3d308b39165929.
// Distributed as part of Age of Decomp under GPL-3.0-or-later.
using System;
using System.Threading.Tasks;
using NxToWin.Dsp;
using NxToWin.Models;

namespace NxToWin.Hardware;

public interface IPhysicalController : IDisposable
{
    string Id { get; }
    string Name { get; }
    string TransportName { get; }
    ushort ProductId { get; }
    bool IsConnected { get; }
    bool IsSwitch2 { get; }
    bool IsProController { get; }
    bool IsLeftJoyCon { get; }
    bool IsRightJoyCon { get; }

    ControllerStateSnapshot LatestState { get; }
    ImuFusionEngine ImuFusion { get; }

    event Action<IPhysicalController, ControllerStateSnapshot>? StateUpdated;
    event Action<IPhysicalController>? Disconnected;

    bool SupportsRumble { get; }
    Task SendHdRumbleAsync(StereoRumble rumble);
    Task SetPlayerLedsAsync(int playerNumber);
    void TriggerGyroCalibration();
    void CenterSticks();
}
