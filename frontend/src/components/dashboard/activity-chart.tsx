import {
  Area,
  AreaChart,
  CartesianGrid,
  XAxis,
  YAxis,
  Tooltip,
  ResponsiveContainer,
} from 'recharts'

const data = [
  { day: 'Mon', matched: 45, pending: 20 },
  { day: 'Tue', matched: 120, pending: 35 },
  { day: 'Wed', matched: 85, pending: 28 },
  { day: 'Thu', matched: 230, pending: 65 },
  { day: 'Fri', matched: 180, pending: 45 },
  { day: 'Sat', matched: 320, pending: 80 },
  { day: 'Sun', matched: 150, pending: 42 },
]

export function ActivityChart() {
  return (
    <div className="glass-card p-6 rounded-xl">
      <div className="flex items-center justify-between mb-6">
        <div>
          <h3 className="text-lg font-semibold text-foreground">
            Daily Match Activity
          </h3>
          <p className="text-sm text-muted-foreground">Last 7 Days</p>
        </div>
        <div className="flex items-center gap-4">
          <div className="flex items-center gap-2">
            <div className="w-3 h-3 rounded-full bg-teal" />
            <span className="text-xs text-muted-foreground">
              Matched Offers
            </span>
          </div>
          <div className="flex items-center gap-2">
            <div className="w-3 h-3 rounded-full bg-amber" />
            <span className="text-xs text-muted-foreground">
              Pending Requests
            </span>
          </div>
        </div>
      </div>

      <div className="h-[280px]">
        <ResponsiveContainer width="100%" height="100%">
          <AreaChart
            data={data}
            margin={{ top: 10, right: 10, left: 0, bottom: 0 }}
          >
            <defs>
              <linearGradient id="colorMatched" x1="0" y1="0" x2="0" y2="1">
                <stop
                  offset="5%"
                  stopColor="hsl(183, 100%, 50%)"
                  stopOpacity={0.3}
                />
                <stop
                  offset="95%"
                  stopColor="hsl(183, 100%, 50%)"
                  stopOpacity={0}
                />
              </linearGradient>
              <linearGradient id="colorPending" x1="0" y1="0" x2="0" y2="1">
                <stop
                  offset="5%"
                  stopColor="hsl(43, 100%, 50%)"
                  stopOpacity={0.3}
                />
                <stop
                  offset="95%"
                  stopColor="hsl(43, 100%, 50%)"
                  stopOpacity={0}
                />
              </linearGradient>
            </defs>
            <CartesianGrid
              strokeDasharray="3 3"
              stroke="hsl(220, 16%, 18%)"
              vertical={false}
            />
            <XAxis
              dataKey="day"
              axisLine={false}
              tickLine={false}
              tick={{ fill: 'hsl(215, 20%, 55%)', fontSize: 12 }}
            />
            <YAxis
              axisLine={false}
              tickLine={false}
              tick={{ fill: 'hsl(215, 20%, 55%)', fontSize: 12 }}
            />
            <Tooltip
              contentStyle={{
                backgroundColor: 'hsl(220, 18%, 8%)',
                border: '1px solid hsl(220, 16%, 22%)',
                borderRadius: '8px',
                boxShadow: '0 4px 20px rgba(0, 0, 0, 0.3)',
              }}
              labelStyle={{ color: 'hsl(210, 40%, 98%)' }}
              itemStyle={{ color: 'hsl(215, 20%, 55%)' }}
            />
            <Area
              type="monotone"
              dataKey="matched"
              stroke="hsl(183, 100%, 50%)"
              strokeWidth={2}
              fill="url(#colorMatched)"
              dot={{ fill: 'hsl(183, 100%, 50%)', strokeWidth: 0, r: 4 }}
              activeDot={{
                r: 6,
                fill: 'hsl(183, 100%, 50%)',
                stroke: 'hsl(220, 18%, 8%)',
                strokeWidth: 2,
              }}
            />
            <Area
              type="monotone"
              dataKey="pending"
              stroke="hsl(43, 100%, 50%)"
              strokeWidth={2}
              fill="url(#colorPending)"
              dot={{ fill: 'hsl(43, 100%, 50%)', strokeWidth: 0, r: 4 }}
              activeDot={{
                r: 6,
                fill: 'hsl(43, 100%, 50%)',
                stroke: 'hsl(220, 18%, 8%)',
                strokeWidth: 2,
              }}
            />
          </AreaChart>
        </ResponsiveContainer>
      </div>
    </div>
  )
}
