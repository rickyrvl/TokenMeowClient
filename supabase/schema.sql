-- TokenMeow Client Database Schema

-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Machines table to store compute node information
CREATE TABLE IF NOT EXISTS machines (
    id UUID DEFAULT uuid_generate_v4(),
    machine_id VARCHAR(255) PRIMARY KEY,
    endpoint VARCHAR(255),
    status VARCHAR(50) DEFAULT 'offline',
    last_seen TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Enable Row Level Security
ALTER TABLE machines ENABLE ROW LEVEL SECURITY;

-- Policies for machines table
CREATE POLICY "Allow anonymous read" ON machines FOR SELECT USING (true);
CREATE POLICY "Allow authenticated insert" ON machines FOR INSERT WITH CHECK (true);
CREATE POLICY "Allow authenticated update" ON machines FOR UPDATE USING (true);

-- Metrics raw table for real-time performance data
CREATE TABLE IF NOT EXISTS metrics_raw (
    id UUID DEFAULT uuid_generate_v4(),
    machine_id VARCHAR(255) NOT NULL,
    tokens_per_second DECIMAL(10, 4),
    gpu_usage DECIMAL(5, 2),
    recorded_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    CONSTRAINT fk_machine FOREIGN KEY (machine_id) REFERENCES machines(machine_id) ON DELETE CASCADE
);

-- Enable Row Level Security
ALTER TABLE metrics_raw ENABLE ROW LEVEL SECURITY;

-- Policy for metrics_raw
CREATE POLICY "Allow anonymous read" ON metrics_raw FOR SELECT USING (true);
CREATE POLICY "Allow authenticated insert" ON metrics_raw FOR INSERT WITH CHECK (true);

-- Indexes for better query performance
CREATE INDEX IF NOT EXISTS idx_machines_status ON machines(status);
CREATE INDEX IF NOT EXISTS idx_machines_last_seen ON machines(last_seen DESC);
CREATE INDEX IF NOT EXISTS idx_metrics_machine_id ON metrics_raw(machine_id);
CREATE INDEX IF NOT EXISTS idx_metrics_recorded_at ON metrics_raw(recorded_at DESC);

-- Function to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger for machines updated_at
DROP TRIGGER IF EXISTS update_machines_updated_at ON machines;
CREATE TRIGGER update_machines_updated_at
    BEFORE UPDATE ON machines
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
