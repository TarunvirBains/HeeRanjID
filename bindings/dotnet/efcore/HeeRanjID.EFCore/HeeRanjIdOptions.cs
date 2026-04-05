namespace HeeRanjID.EFCore;

public class HeeRanjIdOptions
{
    public int NodeId { get; set; } = 1;

    public static HeeRanjIdOptions FromEnvironment()
    {
        var nodeIdStr = Environment.GetEnvironmentVariable("NODE_ID");
        if (string.IsNullOrEmpty(nodeIdStr))
            throw new InvalidOperationException("NODE_ID environment variable must be set");

        return new HeeRanjIdOptions
        {
            NodeId = int.Parse(nodeIdStr)
        };
    }
}
